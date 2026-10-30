/*
name: Archfiend DeathLord
description: This script will farm the "Archfiend DeathLord" armor.
tags: archfiend,afdl2,afdl,deathlord,armor,nulgath
*/
// Port of Scripts/Nation/Various/ArchfiendDeathLord.cs.

import { bot, choices, defineScriptOptions, option } from "turu";
import { Core, configEnum, configOr } from "../../core/CoreBots";
import { Fiendshard_Story } from "../../story/nation/Fiendshard";
import { CoreNation } from "../CoreNation";
import { WillpowerExtraction } from "../afdl/WillpowerExtraction";

const Fiendshard = new Fiendshard_Story();
const Nation = new CoreNation();
const Willpower = new WillpowerExtraction();

export class ArchfiendDeathLord {
    getArm(OnlyArmor: boolean, Reward: RewardChoice): void {
        if (Reward !== RewardChoice.All && Core.checkInventory(Reward, 1, { toInv: false }))
            return;
        else if (OnlyArmor && Reward === RewardChoice.All)
            Core.logger(
                "With \"OnlyArmor\" Please Select the \"Archfiend DeathLord\" Option from the list.", "", { messageBox: true, stopBot: true });

        Fiendshard.fiendshard_QuestlineP1();

        if (Reward === RewardChoice.All) {
            Core.logger(`Section: Farm all`);
            let skipFirstItem = true;
            for (const item of Object.values(RewardChoice).filter((v): v is RewardChoice => typeof v === "number")) {
                const itemName = RewardChoice[item];
                bot.drops.add(itemName!.replaceAll("_", " "));

                if (skipFirstItem) {
                    skipFirstItem = false;
                    continue;
                }

                if (Core.checkInventory(item, 1, { toInv: false }))
                    continue;

                Core.farmingLogger(itemName!.replaceAll("_", " "), 1);

                while (!bot.shouldExit && !Core.checkInventory(item, 1, { toInv: false })) {
                    Core.ensureAccept(7900);
                    Nation.farmBloodGem(20);
                    Nation.farmUni13(5);
                    Nation.farmTotemofNulgath(3);
                    Nation.farmVoucher({ member:false });
                    Nation.farmDiamondofNulgath(150);
                    Nation.farmGemofNulgath(50);
                    Willpower.unidentified34(10);
                    Core.ensureComplete(7900, item);

                    bot.wait.forPickup(item);
                }
            }
        } else {
            Core.logger(`Section: AFDL2 (Archfiend DeathLord)`);
            Core.farmingLogger(Reward.toString().replaceAll("_", " "), 1);
            while (!bot.shouldExit && !Core.checkInventory(Reward, 1, { toInv: false })) {
                Core.ensureAccept(7900);
                Nation.farmBloodGem(20);
                Nation.farmUni13(5);
                Nation.farmTotemofNulgath(3);
                Nation.farmVoucher({ member:false });
                Nation.farmDiamondofNulgath(150);
                Nation.farmGemofNulgath(50);
                Willpower.unidentified34(10);
                Core.ensureComplete(7900, Reward);

                bot.wait.forPickup(Reward);
            }
        }
    }
}

export enum RewardChoice {
    All = 1,
    Archfiend_DeathLord = 54366,
    Champion_Ender_of_Nulgath = 54377,
    Doomblade_of_Genocide = 54379,
    Deathlord_Blade_of_Nulgath = 54380,
    Archfiend_Annihilator_of_Death = 54381,
    Corpse_Hijacker_of_Nulgath = 54382,
    Soul_Jacker_of_Nulgath = 54383,
    Archfiendish_Spear_of_Death = 54384,
    Undeathly_SoulReaper_of_Nulgath = 54385,
    Dual_FiendLords_Claymores = 54386,
}

export const scriptOptions = defineScriptOptions("ArchfiendDeathLord", [
    option.bool("OnlyArmor", "Only get the Armor?", "Whether to only get the Armor or all quest rewards", false),
    option.choice("RewardChoice", "Choose Your Reward", "", choices(RewardChoice), "All"),
    Core.skipOptions,
], true);

export default function main(): void {
        Core.setOptions();

        new ArchfiendDeathLord().getArm(configOr("OnlyArmor", false), configEnum("RewardChoice", RewardChoice, 0 as RewardChoice));

        Core.setOptions(false);
}
