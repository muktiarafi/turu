/*
name: M4 batched reads check
description: Compares turu.swf's snapshot, getMany and selectArrayObjectsWhere with the single reads they replace, in game; logs MATCH or DIFF for each.
*/
import { bot } from "turu";

export default function main(): void {
    const call = (name: string, ...args: unknown[]): string => bot.flash.call(name, ...args);
    const parse = (raw: string): unknown => (raw.trim() === "" ? null : JSON.parse(raw));
    const same = (label: string, a: unknown, b: unknown): void => {
        const left = JSON.stringify(a);
        const right = JSON.stringify(b);
        bot.log(`${left === right ? "MATCH" : "DIFF "} ${label}: ${left.slice(0, 120)}${left === right ? "" : ` vs ${right.slice(0, 120)}`}`);
    };

    // snapshot without the target, field by field.
    const snap = parse(call("snapshot", false)) as Record<string, unknown>;
    same("loggedIn", snap.loggedIn, call("isLoggedIn") === "true");
    same("world", snap.world, call("isNull", "world") !== "true");
    same("avatar", snap.avatar, call("isNull", "world.myAvatar") !== "true");
    same("state", snap.state, bot.flash.get("world.myAvatar.dataLeaf.intState"));
    same("rank", snap.rank, bot.flash.get("world.myAvatar.objData.iRank"));
    const loaded =
        (bot.flash.get<number>("world.myAvatar.items.length") ?? 0) > 0 &&
        bot.flash.get("world.mapLoadInProgress") === false &&
        bot.flash.callGameFunction("world.myAvatar.pMC.artLoaded") === true;
    same("loaded", snap.loaded, loaded);
    same("target left alone", snap.target, null);

    // snapshot with the target, against getTargetMonster, after targeting a monster here.
    call("attackMonsterName", "*");
    bot.sleep(500);
    const withTarget = parse(call("snapshot", true)) as Record<string, unknown>;
    const target = parse(call("getTargetMonster")) as Record<string, unknown>;
    same("target", withTarget.target ?? {}, target);
    call("untargetSelf");

    // getMany against get.
    const paths = {
        gold: "world.myAvatar.objData.intGold",
        cell: "world.strFrame",
        map: "world.strMapName",
        missing: "world.noSuchThing.atAll",
    };
    const many = bot.flash.getMany(paths);
    for (const [key, path] of Object.entries(paths)) {
        same(`getMany ${key}`, many[key as keyof typeof paths], bot.flash.get(path) ?? null);
    }

    // selectArrayObjectsWhere against filtering the whole inventory.
    const items = bot.flash.get<Record<string, unknown>[]>("world.myAvatar.items") ?? [];
    const first = items[0];
    if (first) {
        const byName = parse(call("selectArrayObjectsWhere", "world.myAvatar.items", { sName: ` ${String(first.sName).toUpperCase()} ` }));
        same("by name", byName, items.filter((i) => String(i.sName).toLowerCase() === String(first.sName).toLowerCase()));
        const byId = parse(call("selectArrayObjectsWhere", "world.myAvatar.items", { ItemID: Number(first.ItemID) }));
        same("by ID", byId, items.filter((i) => Number(i.ItemID) === Number(first.ItemID)));
    }
    const worn = parse(call("selectArrayObjectsWhere", "world.myAvatar.items", { bEquip: true, sType: "Class" }));
    same("worn class", worn, items.filter((i) => (i.bEquip === true || i.bEquip === 1 || i.bEquip === "1") && String(i.sType) === "Class"));
    same("not an array", call("selectArrayObjectsWhere", "world.strFrame", { sName: "x" }), "");
}
