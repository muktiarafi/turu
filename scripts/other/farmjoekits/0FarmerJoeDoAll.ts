/*
name: FarmerJoeKit0DoAll
description: a start to mid-game autoprogression to setup for near endgame content
tags: 0farmerjoedoall, farmerjoe, joedoall, doall, Joe, joe mama
*/
// Port of Scripts/Other/FarmJoeKits/0FarmerJoeDoAll.cs. The C# file includes the whole FarmerJoe
// tree (156 files); here CoreFarmerJoe.ts imports what its DoAll reaches.

import { Core } from "../../core/CoreBots";
import { CoreFarmerJoe, scriptOptions as farmerJoeOptions } from "./CoreFarmerJoe";

const CFJ = new CoreFarmerJoe();
// const BAI = new BankAllItems(); // Tools/BankAllItems.cs, only used by the commented-out BankAll below

// C#: `OptionsStorage = CFJ.OptionsStorage`, `Options = CFJ.Options`, `DontPreconfigure = true`.
export const scriptOptions = farmerJoeOptions;

export class FarmerJoeKit0DoAll {
    fjDoAll(): void {
        CFJ.doAll();

        // BAI.bankAll();
    }
}

export default function main(): void {
    Core.setOptions();

    new FarmerJoeKit0DoAll().fjDoAll();

    Core.setOptions(false);
}
