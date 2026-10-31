//! Turu: a character's look from its account.aq.com character page
//! (`https://account.aq.com/CharPage?id=<name>`), for Cosmetics' Copy from Character Page.
//!
//! The page embeds AQW's character viewer (`chardetail/characterB.swf`) and hands it the whole
//! look in its `FlashVars`: each slot's file, link, item ID and name (`strCustArmorFile`,
//! `CustArmorID`, ...; the `Cust` item is the one worn over the plain one), the gender and the
//! six colors. The files are the catalog's (`LaeWeddingSuit.swf`, `items/helms/...`), so the
//! items equip as they are. An unknown name gets a page without `FlashVars` ("Not Found!").

use super::avatar::{ColorChange, Colors};
use super::{CosmeticItem, CosmeticSet, Slot};
use std::collections::HashMap;
use std::time::Duration;

/// A character's look as its page shows it.
#[derive(Debug, Clone)]
pub struct CharPage {
    /// The name as the page spells it (`strName`).
    pub name: String,
    /// The items, and the gender when the page has one.
    pub set: CosmeticSet,
    pub colors: ColorChange,
}

/// The page of a character name.
pub fn url(name: &str) -> String {
    format!(
        "https://account.aq.com/CharPage?id={}",
        super::escape_data(name.trim())
    )
}

/// Downloads and reads a character's page; the error is a message for the window.
pub fn fetch(name: &str) -> Result<CharPage, String> {
    if name.trim().is_empty() {
        return Err("Please enter a character name.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("Turu/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let html = client
        .get(url(name))
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
        .map_err(|e| format!("Couldn't read the character page: {e}"))?;
    parse(&html).ok_or_else(|| format!("Character '{}' was not found.", name.trim()))
}

/// The look in a character page's HTML; `None` when it has no `FlashVars` (no such
/// character).
pub fn parse(html: &str) -> Option<CharPage> {
    let vars = flash_vars(html)?;
    let get = |key: &str| vars.get(key).map(String::as_str).unwrap_or("");
    let name = get("strName").to_owned();
    let mut set = CosmeticSet::named(format!("{name}'s Look"));
    set.gender = match get("strGender")
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
    {
        Some(g @ ('M' | 'F')) => Some(g),
        _ => None,
    };
    for slot in Slot::ALL {
        set.set(slot, item(&vars, slot));
    }
    let colors = ColorChange(Colors::FIELDS.map(|field| get(field).trim().parse().ok()));
    Some(CharPage { name, set, colors })
}

/// The slot's worn item: the `Cust` one when there is one, else the plain one.
fn item(vars: &HashMap<String, String>, slot: Slot) -> Option<CosmeticItem> {
    // Page keys: ID, file, link, name, and the plain armor's names (it's the class).
    let (word, category) = match slot {
        Slot::Armor => ("Armor", "Armor"),
        Slot::Helm => ("Helm", "Helm"),
        Slot::Cape => ("Cape", "Cape"),
        Slot::Weapon => ("Weapon", ""),
        Slot::Pet => ("Pet", "Pet"),
        Slot::Rune => ("Misc", "Misc"),
    };
    let get = |key: &str| vars.get(key).map(|v| v.trim()).unwrap_or("");
    let has_file = |file: &str| !file.is_empty() && !file.eq_ignore_ascii_case("none");
    let cust = format!("strCust{word}");
    let (prefix, id_key, group, file_key, link_key) = if has_file(get(&format!("{cust}File"))) {
        (cust, format!("Cust{word}ID"), slot.group(), None, None)
    } else if slot == Slot::Armor {
        (
            "strArmor".to_owned(),
            "ArmorID".to_owned(),
            "ar",
            Some("strClassFile"),
            Some("strClassLink"),
        )
    } else {
        (
            format!("str{word}"),
            format!("{word}ID"),
            slot.group(),
            None,
            None,
        )
    };
    let file = get(file_key
        .map_or(format!("{prefix}File"), str::to_owned)
        .as_str())
    .to_owned();
    if !has_file(&file) {
        return None;
    }
    let link = get(link_key
        .map_or(format!("{prefix}Link"), str::to_owned)
        .as_str());
    let category = match slot {
        Slot::Weapon => get(&format!("{prefix}Type")).to_owned(),
        Slot::Armor if group == "ar" => "Class".to_owned(),
        _ => category.to_owned(),
    };
    Some(CosmeticItem {
        id: get(&id_key).parse().unwrap_or(0),
        name: get(&format!("{prefix}Name")).to_owned(),
        path: file,
        link: if link.eq_ignore_ascii_case("none") {
            String::new()
        } else {
            link.to_owned()
        },
        item_group: group.to_owned(),
        category,
        meta: String::new(),
    })
}

/// The viewer's `FlashVars` (`<param name="FlashVars" value="...">` or `<embed flashvars="...">`)
/// as decoded key/value pairs.
fn flash_vars(html: &str) -> Option<HashMap<String, String>> {
    let lower = html.to_ascii_lowercase();
    let start = [r#"name="flashvars" value=""#, r#"flashvars=""#]
        .iter()
        .find_map(|marker| lower.find(marker).map(|at| at + marker.len()))?;
    let end = start + html[start..].find('"')?;
    let vars: HashMap<String, String> = unescape_html(&html[start..end])
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_owned(), percent_decode(value)))
        .collect();
    (!vars.is_empty()).then_some(vars)
}

fn unescape_html(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// `%XX` escapes as UTF-8; `+` stays a plus (Flash reads FlashVars that way: "Lae + Thay").
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The viewer part of `CharPage?id=lae` as it was on 2026-11-01.
    pub(crate) const LAE: &str = r#"<object classid="clsid:D27CDB6E-AE6D-11cf-96B8-444553540000" width="715" height="455">
<param name="movie" value="https://game.aq.com/game/gamefiles/etc/chardetail/characterB.swf?v=2" />
<param name="FlashVars" value="&amp;intColorHair=3355443&amp;intColorSkin=16777164&amp;intColorEye=16750848&amp;intColorTrim=16763955&amp;intColorBase=3355443&amp;intColorAccessory=26163&amp;level=100&amp;guild=Hollowborn&amp;ia1=1354416&amp;strGender=M&amp;strHairFile=hair/M/Blank.swf&amp;strHairName=Blank&amp;strName=Lae&amp;intLevel=100 --- Hollowborn Guild&amp;strFaction=Evil&amp;strClassName=Hollowborn&amp;strClassFile=HollowBornEvoker.swf&amp;strClassLink=HollowBornEvoker&amp;strArmorName=HollowBorn Evoker&amp;strWeaponFile=items/swords/sword01.swf&amp;strWeaponLink=&amp;strWeaponType=&amp;strWeaponName=Default Sword&amp;strCapeFile=items/capes/HollowBornRunes.swf&amp;strCapeLink=HollowBornRunes&amp;strCapeName=Hollowborn Rune Cape&amp;strHelmFile=items/helms/VoidBMageHood.swf&amp;strHelmLink=VoidBMageHood&amp;strHelmName=Void BattleMage Hood&amp;strPetFile=items/pets/HBZweibladeNOCCBattlepetr1.swf&amp;strPetLink=HBZweibladeNOCCBattlepet&amp;strPetName=Hollowborn Shadow Shaper%27s Battlepet&amp;strMiscFile=none&amp;strMiscLink=none&amp;strMiscName=&amp;WeaponID=0&amp;CapeID=45951&amp;HelmID=46706&amp;ArmorID=45939&amp;HairID=3&amp;CustWeaponID=156&amp;strCustWeaponFile=items/swords/unarmed.swf&amp;strCustWeaponLink=unarmed&amp;strCustWeaponType=Mace&amp;strCustWeaponName=Unarmed&amp;CustArmorID=89259&amp;strCustArmorFile=LaeWeddingSuit.swf&amp;strCustArmorLink=LaeWeddingSuit&amp;strCustArmorName=Lae + Thay Wedding Attire&amp;CustCapeID=89263&amp;strCustCapeFile=items/capes/HBRingCape.swf&amp;strCustCapeLink=HBRingCape&amp;strCustCapeName=Hollowborn Promise Ring&amp;CustHelmID=89450&amp;strCustHelmFile=items/helms/APIllusionSkinFace.swf&amp;strCustHelmLink=APIllusionSkinFace&amp;strCustHelmName=Apocalyptic Illusionist Unmasked&amp;bgindex=I" />
</object>"#;

    #[test]
    fn reads_the_worn_items_gender_and_colors() {
        let page = parse(LAE).unwrap();
        assert_eq!(page.name, "Lae");
        assert_eq!(page.set.gender, Some('M'));
        let armor = page.set.armor.as_ref().unwrap();
        assert_eq!(
            (
                armor.id,
                armor.path.as_str(),
                armor.link.as_str(),
                armor.item_group.as_str()
            ),
            (89259, "LaeWeddingSuit.swf", "LaeWeddingSuit", "co")
        );
        assert_eq!(armor.name, "Lae + Thay Wedding Attire");
        assert_eq!(page.set.helm.as_ref().unwrap().id, 89450);
        assert_eq!(
            page.set.cape.as_ref().unwrap().path,
            "items/capes/HBRingCape.swf"
        );
        let weapon = page.set.weapon.as_ref().unwrap();
        assert_eq!(
            (
                weapon.id,
                weapon.category.as_str(),
                weapon.item_group.as_str()
            ),
            (156, "Mace", "Weapon")
        );
        let pet = page.set.pet.as_ref().unwrap();
        assert_eq!(pet.name, "Hollowborn Shadow Shaper's Battlepet");
        assert_eq!(pet.link, "HBZweibladeNOCCBattlepet");
        assert!(page.set.misc.is_none(), "strMiscFile=none");
        assert_eq!(
            page.colors.0,
            [
                Some(3355443),
                Some(16763955),
                Some(26163),
                Some(16750848),
                Some(16777164),
                Some(3355443)
            ]
        );
    }

    #[test]
    fn falls_back_to_the_class_and_knows_a_missing_character() {
        let page = parse(r#"<embed flashvars="&amp;strName=Bob&amp;strGender=f&amp;strClassFile=Mage.swf&amp;strClassLink=Mage&amp;strArmorName=Mage&amp;ArmorID=7" />"#)
            .unwrap();
        let armor = page.set.armor.unwrap();
        assert_eq!(
            (armor.item_group.as_str(), armor.category.as_str(), armor.id),
            ("ar", "Class", 7)
        );
        assert_eq!(page.set.gender, Some('F'));
        assert!(page.set.helm.is_none() && page.colors.0.iter().all(Option::is_none));
        assert!(parse(r#"<div class="alert">Not Found!</div>"#).is_none());
        assert_eq!(
            url(" Lae Thay "),
            "https://account.aq.com/CharPage?id=Lae%20Thay"
        );
        assert_eq!(percent_decode("a%27b+c%zz"), "a'b+c%zz");
    }

    /// The real page, over the network: `cargo test -p turu_bot charpage -- --ignored`.
    #[test]
    #[ignore = "reads account.aq.com"]
    fn reads_a_real_character_page() {
        let page = fetch("lae").unwrap();
        assert_eq!(page.name, "Lae");
        assert!(page.set.armor.is_some(), "{page:?}");
        assert!(
            fetch("zzqqxxnotarealname123")
                .unwrap_err()
                .contains("was not found")
        );
    }
}
