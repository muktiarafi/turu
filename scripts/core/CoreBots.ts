/*
name: CoreBots
description: Port of Scripts/CoreBots.cs: the shared helpers every script uses (joining, killing,
    quests, drops, banking, shops, class swapping).
tags: core
*/
/**
 * CoreBots — TypeScript port of `Scripts/CoreBots.cs` for Turu.
 *
 * Use the singleton: `import { Core } from "../core/CoreBots";` (C#: `CoreBots.Instance`).
 *
 * ## Configuration
 * Like in VibeSkua, `Core.setOptions()` overrides the `[Can Change]` fields below from your
 * CoreBots Options file (Options > CoreBots, saved per character as
 * `options/CBO_Storage(<user>).txt`; see {@link CoreBots.readCbo}). Without that file the
 * fields keep the C# defaults. The keys some helpers read from the file while they run
 * (`Nation_SellMemVoucher`, `Nation_ReturnPolicyDuringSupplies`, `UltraAlteonForSupplies`,
 * `doGoldBoost`, ...) can be overridden from a script in {@link CoreBots.cboValues}:
 *
 * ```ts
 * Core.cboValues["Nation_SellMemVoucher"] = "True";
 * ```
 *
 * ## Porting notes
 * - Everything is synchronous. C# background tasks (registered quests, the AFK handler) run as
 *   "handlers" that Core polls from its own waits ({@link CoreBots.sleep}, {@link CoreBots.pause},
 *   {@link CoreBots.waitForTrue}); see {@link CoreBots.registerHandler}.
 * - Members only reachable from `SetOptions`' infrastructure (version checker, telemetry, April
 *   Fools, crash reports, account tags, Butler logs, Windows-only bits) are
 *   dropped or stubbed; each spot says so in a one-line comment.
 * - `ScriptStopped` is never swallowed: every `catch` rethrows it.
 */

import { bot, ClassUseMode, ScriptStopped } from "turu";
import type { InventoryItem, Monster, Quest, QuestItem } from "turu";

// ---------------------------------------------------------------------------------------
// Enums and data types
// ---------------------------------------------------------------------------------------

/** Which configured class {@link CoreBots.equipClass} should equip. */
export enum ClassType {
    Solo,
    Farm,
    Dodge,
    Boss,
    None,
}

/** Player alignment (quest 41's value), see {@link CoreBots.changeAlignment}. */
export enum Alignment {
    Good = 1,
    Evil = 2,
    Chaos = 3,
}

/** A quest reward with its max stack (C# `ItemBase` from `oRewards`). */
export interface RewardItem {
    id: number;
    name: string;
    /** How many the reward gives. */
    quantity: number;
    maxStack: number;
    temp: boolean;
}

/** An entry of the quest's `reward` list (C# `SimpleReward`). `type == 2` = pick-one reward. */
export interface SimpleReward extends RewardItem {
    rate: number;
    type: number;
}

/**
 * A quest as CoreBots uses it: turu's {@link Quest} plus the reward details (max stacks,
 * reward types) that `bot.quests` doesn't expose, read from `world.questTree`.
 */
export interface QuestData extends Omit<Quest, "rewards" | "simpleRewards"> {
    rewards: RewardItem[];
    simpleRewards: SimpleReward[];
}

/** A requirement of a merge-shop item (C# `ShopItem.Requirements`). */
export interface ShopRequirement {
    id: number;
    name: string;
    quantity: number;
}

/** A shop item with the fields CoreBots needs (read from `world.shopinfo.items`). */
export interface CoreShopItem {
    id: number;
    shopItemId: number;
    name: string;
    /** Bundle size (how many one purchase gives). */
    quantity: number;
    maxStack: number;
    category: string;
    upgrade: boolean;
    coins: boolean;
    cost: number;
    faction: string;
    requiredReputation: number;
    requirements: ShopRequirement[];
}

/** Options of {@link CoreBots.join}. */
export interface JoinOptions {
    /** Join the public room when {@link CoreBots.publicDifficult} is on. */
    publicRoom?: boolean;
    /** Join even if already in the map. */
    ignoreCheck?: boolean;
}

/** Options of the kill/hunt helpers. */
export interface KillOptions {
    /** The item is a temporary (quest) item. */
    isTemp?: boolean;
    /** Log the farming progress. */
    log?: boolean;
    /** Use a public room (only when {@link CoreBots.publicDifficult} is on). */
    publicRoom?: boolean;
}

/** Options of {@link CoreBots.logger}. */
export interface LoggerOptions {
    /** C#: also show a message box. Turu logs only. */
    messageBox?: boolean;
    /** Stop the script after logging. */
    stopBot?: boolean;
}

/** A background check Core runs from its waits (C# `Bot.Handlers.RegisterHandler`). */
interface Handler {
    name: string;
    interval: number;
    next: number;
    run: () => void;
}

/** State of the registered-quest loops started by {@link CoreBots.registerQuests}. */
interface QuestLoop {
    quests: QuestData[];
    chooseQuests: Set<number>;
    /** Quest ID -> time (ms) when the turn-in we sent should be checked. */
    inFlight: Map<number, number>;
    stuckCounts: Map<number, number>;
    /** Watchdog threshold (ms). */
    threshold: number;
}

/** Raw JSON from the game: values may be strings, numbers or booleans. */
type Raw = Record<string, any>;

const QUEST_WATCHER = "Quest Watcher";
const QUEST_MANAGER = "Quest Manager";
const QUEST_WATCHDOG = "Quest Watchdog";

// ---------------------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------------------

/** Rethrows `ScriptStopped` (call first in every `catch`). */
export function rethrowIfStopped(error: unknown): void {
    if (error instanceof ScriptStopped) throw error;
}

function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
}

function num(value: unknown): number {
    if (typeof value === "number") return value;
    if (typeof value === "boolean") return value ? 1 : 0;
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : 0;
}

function flag(value: unknown): boolean {
    return value === true || value === 1 || value === "1" || value === "true" || value === "True";
}

function text(value: unknown): string {
    return value === null || value === undefined ? "" : String(value);
}

function unique<T>(values: Iterable<T>): T[] {
    return [...new Set(values)];
}

function equalsIgnoreCase(a: string | null | undefined, b: string | null | undefined): boolean {
    return a !== null && a !== undefined && b !== null && b !== undefined && a.toLowerCase() === b.toLowerCase();
}

/** C# `TextInfo.ToTitleCase(s.ToLower())`: "left" -> "Left", "center" -> "Center". */
function toTitleCase(value: string): string {
    return value
        .toLowerCase()
        .split(" ")
        .map((word) => (word.length > 0 ? word[0].toUpperCase() + word.slice(1) : word))
        .join(" ");
}

/** C# `ItemBase.Category`: the `sType` without spaces, compared case-insensitively. */
function categoryIs(item: { category: string }, category: string): boolean {
    return item.category.replace(/\s/g, "").toLowerCase() === category.toLowerCase();
}

/** "hh:mm:ss" for a duration in ms. */
function formatDuration(ms: number): string {
    const total = Math.floor(ms / 1000);
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total % 3600) / 60))}:${pad(total % 60)}`;
}

/** "HH:mm:ss" of the current time. */
function timeOfDay(): string {
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
}

/** Random integer in [min, max). */
function randomInt(min: number, max: number): number {
    return min + Math.floor(Math.random() * (max - min));
}

/**
 * Name of the function `depth` frames above the caller of this helper (C# `[CallerMemberName]`).
 * Best effort: "" when the engine's stack trace has no name.
 */
function callerName(depth: number): string {
    const frames = (new Error().stack ?? "")
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.startsWith("at "));
    const match = /^at (?:new )?([\w$.]+)/.exec(frames[depth + 1] ?? "");
    const name = match ? match[1].split(".").pop() ?? "" : "";
    return name === "<anonymous>" || name === "anonymous" ? "" : name;
}

/**
 * C# `UtilExtensionsS.FormatForCompare`: normalizes a name (diacritics, quotes, dashes,
 * invisible characters, spaces) and lowercases it.
 */
export function formatForCompare(input: string | null | undefined, caseSensitive = false): string {
    if (!input) return "";
    let result = input.normalize("NFD").trim();
    // RemoveDiacritics: drop non-spacing marks, recompose.
    result = result.replace(/\p{Mn}/gu, "").normalize("NFC");
    result = result.replace(/[‘’‚‛`]/g, "'");
    result = result.replace(/[“”„‟]/g, '"');
    result = result.replace(/[‐‑‒–—―]/g, "-");
    result = result.replace(/[​-‍﻿ ⁠᠎  ]/g, " ");
    // Control characters except \r \n \t (the C# regex intends this).
    result = result.replace(/[^\P{C}\r\n\t]+/gu, "");
    result = result.replace(/\s+/g, " ").trim();
    return caseSensitive ? result : result.toLowerCase();
}

function levenshteinDistance(a: string, b: string): number {
    const matrix: number[][] = [];
    for (let i = 0; i <= a.length; i++) matrix.push([i]);
    for (let j = 0; j <= b.length; j++) matrix[0][j] = j;
    for (let i = 1; i <= a.length; i++)
        for (let j = 1; j <= b.length; j++) {
            const cost = a[i - 1] === b[j - 1] ? 0 : 1;
            matrix[i][j] = Math.min(Math.min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1), matrix[i - 1][j - 1] + cost);
        }
    return matrix[a.length][b.length];
}

function allowedTypoDistance(name: string): number {
    return name.length <= 6 ? 1 : name.length <= 12 ? 2 : 3;
}

/** C# `Monster.Alive`. */
function alive(monster: Monster | null | undefined): boolean {
    return !!monster && monster.hp > 0 && monster.state > 0;
}

function toRewardItem(raw: Raw): RewardItem {
    return {
        id: num(raw.ItemID),
        name: text(raw.sName),
        quantity: num(raw.iQty),
        maxStack: num(raw.iStk),
        temp: flag(raw.bTemp),
    };
}

function toShopItem(raw: Raw): CoreShopItem {
    const turnin: Raw[] = Array.isArray(raw.turnin) ? raw.turnin : [];
    return {
        id: num(raw.ItemID),
        shopItemId: num(raw.ShopItemID),
        name: text(raw.sName),
        quantity: num(raw.iQty),
        maxStack: num(raw.iStk),
        category: text(raw.sType),
        upgrade: flag(raw.bUpg),
        coins: flag(raw.bCoins),
        cost: num(raw.iCost),
        faction: text(raw.sFaction),
        requiredReputation: num(raw.iReqRep),
        requirements: turnin.map((r) => ({ id: num(r.ItemID), name: text(r.sName), quantity: num(r.iQty) })),
    };
}

// ---------------------------------------------------------------------------------------
// Game4000 inventory categories (C# `GameInventoryCategories`)
// ---------------------------------------------------------------------------------------

/**
 * TEMPORARY Game4000 inventory change (port of the C# helper): a snapshot of the game's `InvCat`
 * settings and `world.myAvatar.items`, used to tell which inventory pool (bag/misc/house/class)
 * an item uses and whether it's full.
 */
class GameInventoryCategories {
    private readonly items: Raw[];
    private readonly miscTypes: Set<string>;
    readonly miscSlots: number;

    constructor() {
        const settings = bot.flash.callGameFunction<Raw | null>("loaderInfo.applicationDomain.getDefinition", "InvCat");
        if (!settings || typeof settings !== "object")
            throw new Error("Cannot read the game misc inventory settings.");
        const miscSlots = settings.MISC_SLOTS ?? bot.flash.get<number | null>("world.myAvatar.objData.iBagSlots");
        if (miscSlots === null || miscSlots === undefined)
            throw new Error("The game misc inventory limit is unavailable. Report this in the Discord!");
        this.miscSlots = num(miscSlots);
        if (!Array.isArray(settings.MISC_TYPES)) throw new Error("The game misc inventory types are unavailable.");
        this.miscTypes = new Set((settings.MISC_TYPES as unknown[]).map(text));
        const items = bot.flash.get<Raw[] | null>("world.myAvatar.items");
        if (!items) throw new Error("The game inventory is unavailable.");
        this.items = items;
    }

    private static isClass(item: Raw): boolean {
        return text(item.sType) === "Class";
    }

    private isMisc(item: Raw): boolean {
        const type = item.sType as string | undefined;
        return (
            type !== undefined &&
            type !== null &&
            this.miscTypes.has(type) &&
            !(type === "Item" && item.sMeta !== null && item.sMeta !== undefined && /^\s*[0-9]+\s*$/.test(text(item.sMeta)))
        );
    }

    static isHouse(item: Raw): boolean {
        const type = text(item.sType);
        return num(item.bHouse) === 1 || type === "House" || type === "Wall Item" || type === "Floor Item" || type === "Guild";
    }

    private isBag(item: Raw): boolean {
        return !GameInventoryCategories.isClass(item) && !this.isMisc(item) && !GameInventoryCategories.isHouse(item);
    }

    countBag(): number {
        return this.items.filter((item) => this.isBag(item)).length;
    }

    countMisc(): number {
        return this.items.filter((item) => this.isMisc(item)).length;
    }

    poolOf(item: Raw): string {
        return GameInventoryCategories.isClass(item) ? "class" : GameInventoryCategories.isHouse(item) ? "house" : this.isMisc(item) ? "misc" : "bag";
    }

    private owned(id: number): Raw | undefined {
        return this.items.find((item) => Math.trunc(num(item.ItemID)) === id);
    }

    isFullFor(item: Raw, bagSlots: number, freedSlots = 0): boolean {
        if (GameInventoryCategories.isClass(item)) return false;
        const owned = this.owned(Math.trunc(num(item.ItemID)));
        if (owned && num(owned.iQty) < num(owned.iStk)) return false;
        return this.isMisc(item) ? !owned && this.countMisc() - freedSlots >= this.miscSlots : this.countBag() - freedSlots >= bagSlots;
    }

    /** Match InvCat.freedBy: only consumed stacks from the output pool count. */
    freedBy(item: Raw, quantity: number): number {
        if (this.owned(Math.trunc(num(item.ItemID)))) return 0;
        const pool = this.poolOf(item);
        if (pool !== "bag" && pool !== "misc") return 0;

        let batchSize = num(item.iQty);
        if (batchSize <= 0) batchSize = 1;
        const batches = Math.max(1, Math.floor(quantity / batchSize));
        let freed = 0;
        let tokenSeen = false;
        const tokenId = Math.trunc(num(item.TokID));
        const isToken = num(item.TokID) > 0 && num(item.TokQty) > 0;
        const tokenNeed = Math.trunc(num(item.TokQty) * batches);

        if (Array.isArray(item.turnin)) {
            for (const requirement of item.turnin as Raw[]) {
                const id = Math.trunc(num(requirement.ItemID));
                let consumed = num(requirement.iQty) * batches;
                if (isToken && id === tokenId) {
                    tokenSeen = true;
                    consumed += tokenNeed;
                }
                const owned = this.owned(id);
                if (owned && id !== 45739 && this.poolOf(owned) === pool && (text(owned.sES) === "ar" || num(owned.iQty) <= consumed))
                    freed++;
            }
        }

        if (isToken && !tokenSeen && tokenId !== 45739) {
            const owned = this.owned(tokenId);
            if (owned && this.poolOf(owned) === pool && num(owned.iQty) <= tokenNeed) freed++;
        }
        return freed;
    }
}

// ---------------------------------------------------------------------------------------
// CoreBots
// ---------------------------------------------------------------------------------------

export class CoreBots {
    // ---- [Can Change] configuration (C# defaults; setOptions() reads the CBO file over them) ----

    /** [Can Change] Delay between common actions, 700 is the safe number. */
    actionDelay = 700;
    /** [Can Change] Delay used to get out of combat, 1600 is the safe number. */
    exitCombatDelay = 1600;
    /** [Can Change] Delay between jumping rooms after hunting a monster, increase if you think it is jumping too much. */
    huntDelay = 1000;
    /** [Can Change] How many tries to accept/complete the quest will be sent. */
    acceptandCompleteTries = 20;
    /** [Can Change] How many quests the bot should be able to have loaded at once. */
    loadedQuestLimit = 150;
    /** [Can Change] Whether the bots should also log in AQW's chat. */
    loggerInChat = true;
    /** [Can Change] When enabled, no message boxes will be shown unless absolutely necessary. */
    forceOffMessageboxes = false;
    /** [Can Change] Whether the bots will use private rooms. */
    privateRooms = true;
    /** [Can Change] What private room number the bot should use. */
    privateRoomNumber = 100000;
    /** [Can Change] Use public rooms if the enemy is tough. */
    publicDifficult = false;
    /** [Can Change] If StopLocations.Custom is selected, where to go. */
    customStopLocation = "whitemap";
    /** [Can Change] Whether the player should rest after killing a monster. */
    shouldRest = false;
    /** [Can Change] Bank misc. AC items before starting the bot. */
    bankMiscAC = false;
    /** [Can Change] Bank unenhanced AC gear before starting the bot. */
    bankUnenhancedACGear = false;
    /** [Can Change] Anti lag features (lag killer, invisible monsters). */
    antiLag = true;
    /** [Can Change] Name of your soloing class. */
    soloClass = "";
    /** [Can Change] Mode of soloing class, if it has multiple. */
    soloUseMode: ClassUseMode = ClassUseMode.Base;
    /** [Can Change] Whether you wish to equip solo equipment. */
    soloGearOn = true;
    /** [Can Change] Names of your soloing equipment. */
    soloGear: string[] = [];
    /** [Can Change] Name of your farming class. */
    farmClass = "";
    /** [Can Change] Mode of farming class, if it has multiple. */
    farmUseMode: ClassUseMode = ClassUseMode.Base;
    /** [Can Change] Whether you wish to equip farm equipment. */
    farmGearOn = true;
    /** [Can Change] Names of your farming equipment. */
    farmGear: string[] = [];
    /** [Can Change] Name of your dodge class. */
    dodgeClass = "";
    /** [Can Change] Mode of dodge class, if it has multiple. */
    dodgeUseMode: ClassUseMode = ClassUseMode.Base;
    /** [Can Change] Whether you wish to equip dodge equipment. */
    dodgeGearOn = true;
    /** [Can Change] Names of your dodge equipment. */
    dodgeGear: string[] = [];
    /** [Can Change] Name of your bossing class. */
    bossClass = "";
    /** [Can Change] Mode of boss class, if it has multiple. */
    bossUseMode: ClassUseMode = ClassUseMode.Base;
    /** [Can Change] Whether you wish to equip bossing equipment. */
    bossGearOn = true;
    /** [Can Change] Names of your bossing equipment. */
    bossGear: string[] = [];
    /** [Can Change] Member status (set by {@link setOptions} from `iUpgDays`). */
    isMember = false;
    autoEnhance = true;
    bestGear = false;

    /**
     * CoreBots Options values set from a script, e.g. `{ Nation_SellMemVoucher: "True" }`. They
     * win over the CBO file's (C# `CBOBool`/`CBOString` read only the file).
     */
    cboValues: Record<string, string> = {};

    /** The CBO file's lines, read by {@link readCbo} (C# `CBOList`). */
    private cboLines: string[] = [];

    private static _instance: CoreBots | null = null;

    /** The shared instance (C# `CoreBots.Instance`). */
    static get instance(): CoreBots {
        return (CoreBots._instance ??= new CoreBots());
    }

    /**
     * Called with the class name after {@link equipClass} equips a configured class
     * (C# `static event OnEquipClass`; CoreAdvanced hooks auto-enhance here).
     */
    static readonly onEquipClass: ((className: string) => void)[] = [];

    // C# IsVibeSkuaAvalonia, DiscordLink, the version checker's HttpClient and RunSync: dropped
    // (version check / auto-update infrastructure).

    private scriptStartTime: number | null = null;

    /**
     * Set common bot options to desired value.
     *
     * Ported: login wait, test-server check, social privacy (CBO `IncognitoMode`), member status,
     * bot/Lite/drop options, safe start map, bank load, AFK handler, prison detector, anti-lag,
     * start-up banking, skills for the current class and the run-time log on `setOptions(false)`.
     * Dropped: version check, Bot.Config, telemetry (CollectData), account tags, one-time messages
     * (Discord/Christmas/April Fools/user-specific), Butler logs, the quest-limit handler (needs an
     * object `setGameObject`) and anti-counter-attack (`Bot.UltraBossHelper`). Starting also
     * registers {@link stopBotEvent}, which goes to the stop location when the script is stopped.
     * @param changeTo Value the options will be changed to.
     */
    setOptions(changeTo = true, _disableClassSwap = false): void {
        // C# EnforceInvariantCulture / SkuaVersionCheckerAsync: dropped.

        // Clear Handlers incase script starts a 2nd time somehow whilst the script is running
        this.clearHandlers();

        if (changeTo) {
            bot.sleep(1500);
            this.scriptStartTime = Date.now();
            this.logger("Bot Started", "SetOptions");

            if (bot.options.joinMapTries === 0 || bot.options.joinMapTries === 1) {
                this.logger(
                    `You set Join Map Tries to ${bot.options.joinMapTries}. \n\nAre you retarded? \n\nResetting it to 3.`,
                    "SetOptions",
                    { messageBox: true },
                );
                bot.options.joinMapTries = 3;
            }

            // C# Bot.Config.Configure() for scripts with options: not applicable.

            let loadAttempts = 0;
            while (bot.player.loggedIn && !bot.player.loaded && !bot.shouldExit) {
                if (loadAttempts >= 30) {
                    this.logger("Player stuck in spirit state after 30s, reconnecting.", "SetOptions");
                    bot.options.autoRelogin = true;
                    // API gap: no Bot.Servers.Relogin; log out and let auto-relogin reconnect.
                    this.logout();
                    loadAttempts = 0;
                    bot.sleep(1000);
                    continue;
                }
                bot.sleep(1000);
                loadAttempts++;
            }

            // Only stop AFTER retries exhausted
            if (!bot.player.loggedIn) {
                this.logger("Please log-in before starting the bot.", "SetOptions", { messageBox: true, stopBot: true });
                return;
            }
        }

        // C# Bot.UltraBossHelper.EnableCounterAttack(): API gap, dropped.

        if (changeTo && bot.player.loggedIn && bot.player.serverIp) {
            if (bot.player.serverIp === "sockpuppet.aq.com")
                this.logger(
                    "You're current on a test server!! This will cause issues with scripts\n" +
                        "please logout and login to a non-test server manually\n" +
                        "*or* set your manager to a non-Test server",
                    "SetOptions",
                    { stopBot: true },
                );
        }

        this.readCbo();

        // Social Privacy Options
        const isStarting = changeTo;
        if (this.cboBool("IncognitoMode") !== true) {
            if (isStarting) this.logger("Incognito Mode in CBO is off. Skipping privacy settings.", "SetOptions");
        } else {
            const disabling = isStarting;
            let warned = false;
            const settings: [string, string][] = [
                ["bGoto", "Goto"],
                ["bParty", "Party invites"],
                ["bFriend", "Friend invites"],
                ["bDuel", "Duel invites"],
                ["bGuild", "Guild invites"],
                ["bWhisper", "Whisper"],
            ];
            for (const [key, label] of settings) {
                // C#: Goto is only toggled for the Butler script.
                if (label === "Goto") continue;

                const current = bot.flash.get<boolean>(`uoPref.${key}`) === true;
                if (disabling ? current : !current) {
                    if (disabling && !warned) {
                        this.logger('[SetOptions] Turning certain "Social" options off to help protect you', "SetOptions");
                        warned = true;
                    }
                    this.logger(`[SetOptions] ${disabling ? "Turning off" : "Re-enabling"}: ${label}`, "SetOptions");
                    this.sendPackets(`%xt%zm%cmd%1%uopref%${key}%${String(!disabling)}%`);
                    bot.sleep(500);
                }
            }
        }

        // Set the member status
        this.isMember = this.isUpgraded();

        // Common Options
        bot.options.rejectAllDrops = false;
        bot.options.privateRooms = false;
        bot.options.attackWithoutTarget = false;
        bot.options.questAcceptAndCompleteTries = this.acceptandCompleteTries;
        bot.options.autoRelogin = true;
        bot.options.safeTimings = changeTo;
        bot.options.restPackets = changeTo && this.shouldRest;
        bot.options.infiniteRange = changeTo;
        bot.options.skipCutscenes = changeTo;

        // Lite Options
        bot.lite.reacceptQuest = false;
        bot.lite.disableRedWarning = true;
        bot.lite.characterSelectScreen = false;
        bot.lite.untargetDead = true;
        bot.lite.untargetSelf = true;
        bot.lite.smoothBackground = true;
        bot.lite.showMonsterType = true;
        bot.lite.customDropsUI = true;
        bot.lite.aurasUI = true;
        bot.lite.quantityWarnings = false;
        bot.lite.visualSkillCooldowns = true;
        bot.lite.chatUI = true;
        bot.lite.questLogTurnIns = true;
        bot.lite.disableSoundFx = true;

        // Drop Options
        bot.drops.rejectElse = false;
        bot.drops.stop();
        bot.drops.clear();
        bot.drops.start();

        // C# CollectData(changeTo): telemetry, dropped.

        if (changeTo) {
            // Start scripts safely in the house (or whitemap if the house doesn't exist) if the start map is battleon
            if (["battleon", "oaklore", "bludrutbrawl"].some((m) => equalsIgnoreCase(bot.map.name, m))) this.goHome();
            // Open Bank on startup ensuring current window is `Bank`, then load the bank information.
            if (bot.flash.get<string>("ui.mcPopup.currentLabel") !== "Bank") bot.bank.open();
            bot.bank.load();
            bot.bank.loaded = true;

            // C# AutoAddTags() (Bot.Accounts) and the Christmas one-time message: dropped.
        }

        if (changeTo) {
            this.setOptionsBackground(changeTo);

            bot.options.huntDelay = this.huntDelay;
            if (this.bankMiscAC) this.bankACMisc();
            if (this.bankUnenhancedACGear) this.bankACUnenhancedGear();

            this.equipmentBeforeBot.push(...bot.inventory.items.filter((i) => i.equipped).map((x) => x.name));

            const currentClassName = bot.player.currentClass ?? "generic";

            this.usingSoloGeneric = this.soloClass.toLowerCase() === "generic";
            this.usingFarmGeneric = this.farmClass.toLowerCase() === "generic";
            this.usingDodgeGeneric = this.dodgeClass.toLowerCase() === "generic";
            this.usingBossGeneric = this.bossClass.toLowerCase() === "generic";

            bot.skills.startAdvanced(
                currentClassName,
                false,
                currentClassName === this.soloClass
                    ? this.soloUseMode
                    : currentClassName === this.farmClass
                      ? this.farmUseMode
                      : currentClassName === this.bossClass
                        ? this.bossUseMode
                        : currentClassName === this.dodgeClass
                          ? this.dodgeUseMode
                          : ClassUseMode.Base,
            );
            // C# Bot.Events.ScriptStopping += StopBotEvent (kept to one subscription here).
            this.removeStopBotEvent?.();
            this.removeStopBotEvent = bot.events.on("scriptStopping", (e) => this.stopBotEvent(e));
            // C# killed-monster listeners (CleanKilledMonstersList, ...): dropped.

            this.logger("Bot Configured", "SetOptions");
        }

        if (!changeTo && this.scriptStartTime !== null) {
            bot.drops.clear();
            this.logger(`Script ran for ${formatDuration(Date.now() - this.scriptStartTime)}`, "SetOptions");
            this.scriptStartTime = null;
        }
    }

    /** The part of SetOptions C# runs in a background task (`SetOptionsAsync`). */
    private setOptionsBackground(changeTo: boolean): void {
        // C# Discord one-time prompt and Butler log cleanup: dropped.

        // AFK Handler: logs out after 5 minutes AFK (C# sleeps 300 s on a handler thread; here the
        // handler remembers when AFK started instead of blocking the script).
        bot.send.packet("%xt%zm%afk%1%false%");
        this.sleep();
        let afkSince: number | null = null;
        this.registerHandler("AFK Handler", 5000, () => {
            if (!bot.player.afk) {
                if (afkSince !== null) this.logger("afk cleared", "AFK Handler");
                afkSince = null;
                return;
            }
            if (afkSince === null) {
                afkSince = Date.now();
                this.logger("afk detected, starting timer", "AFK Handler");
            } else if (Date.now() - afkSince >= 300000) {
                this.logger("300000 milisecond afk handler turned on, send logs", "AFK Handler");
                bot.options.autoRelogin = true;
                this.logout();
                this.logger("corebots.cs afk handler logged out", "AFK Handler");
                afkSince = null;
            }
        });

        // C# Quest-Limit Handler: clears `world.questTree` (an object) when more than
        // LoadedQuestLimit quests are loaded. API gap: `bot.flash.set` can't assign an object.

        // Prison Detector
        const offPrison = bot.events.on("mapChanged", ({ map }) => {
            if (map.toLowerCase() === "prison" && !this.joinedPrison && !this.prisonListenerActive) {
                this.prisonListenerActive = true;
                bot.options.autoRelogin = false;
                this.logout();
                const message =
                    "You were teleported to /prison by someone other than the bot. We disconnected you and stopped the bot out of precaution.\n" +
                    "Be ware that you might have received a ban, as this is a method moderators use to see if you're botting." +
                    (!this.privateRooms || this.privateRoomNumber < 1000 || this.publicDifficult
                        ? "\nGuess you should have stayed out of public rooms!"
                        : "");
                this.logger(message, "PrisonDetector");
                offPrison();
                bot.stop();
            }
            offPrison();
        });

        // Anti-lag option
        if (this.antiLag) {
            bot.options.lagKiller = changeTo;

            // Some maps are codded horrible and the animations can cause lag or freezes, so we'll turn all the animations off
            bot.lite.freezeMonsterPosition = changeTo;
            bot.lite.disableMonsterAnimation = changeTo;
            bot.lite.disableDamageStrobe = changeTo;
            bot.lite.disableSelfAnimation = changeTo;
            bot.lite.disableWeaponAnimation = changeTo;
            bot.lite.disableSkillAnimation = changeTo;
            bot.lite.disableAuraAnimations = changeTo;

            if (bot.flash.get<boolean>("ui.monsterIcon.redX.visible") !== true) bot.flash.callGameFunction("world.toggleMonsters");
        }

        // C# AprilFools() / UserSpecificMessages(): dropped.
    }

    /** Whether the player is a Member (set during {@link setOptions}). */
    isUpgraded(): boolean {
        const days = bot.flash.get<number | string | null>("world.myAvatar.objData.iUpgDays");
        const parsed = Number.parseInt(text(days), 10);
        return Number.isFinite(parsed) && parsed > 0;
    }

    /** Item names {@link toBank} and the AC-banking helpers must never bank. */
    bankingBlackList: string[] = [];

    private readonly equipmentBeforeBot: string[] = [];
    private removeStopBotEvent: (() => void) | null = null;

    /**
     * Goes to your house, or to /whitemap when you have no house equipped (where scripts start
     * when you are in battleon, and the "home"/"house" stop location). C# also rewrites the
     * house's moveToArea packet to hide the username; dropped.
     */
    private goHome(): void {
        if (bot.house.items.some((h) => h.equipped)) {
            bot.send.packet(`%xt%zm%house%1%${this.username()}%`);
            bot.wait.forMapLoad("house");
        } else bot.send.packet(`%xt%zm%cmd%1%tfer%${this.username()}%whitemap-${this.privateRoomNumber}%`);
    }

    /**
     * Stops the bot and moves you to the stop location ("Map after stopping the Bot" in
     * Options > CoreBots, {@link customStopLocation}): your house for "home"/"house", nowhere for
     * "off"/"none"/..., else that map. Then puts back the gear you wore when the script started.
     * Runs when the script is stopped or fails, not when it finishes (C# `StopBot`).
     */
    private stopBot(crashed: boolean): boolean {
        this.stopBotBackground();
        this.clearHandlers();

        if (bot.player.loggedIn) {
            this.jumpWait();
            this.sleep();

            if (this.customStopLocation.trim().length > 0) {
                const stopLoc = this.customStopLocation.trim().toLowerCase();
                if (["home", "house"].includes(stopLoc)) this.goHome();
                else if (["off", "disabled", "disable", "stop", "same", "currentmap", "bot.map.currentmap", "none", ""].includes(stopLoc)) {
                    // Nothing happens
                } else bot.send.packet(`%xt%zm%cmd%1%tfer%${this.username()}%${stopLoc}-${this.privateRoomNumber}%`);

                if (this.equipmentBeforeBot.length > 0) {
                    const pvpMaps = [
                        "bludrutbrawl",
                        "darkoviapvp",
                        "dagepvp",
                        "deathpitbrawl",
                        "frostbrawl",
                        "chaosbrawl",
                        "doomarenaa",
                        "doomarenab",
                        "doomarenac",
                        "doomarenad",
                    ];

                    if (pvpMaps.includes(bot.map.name)) this.join("whitemap");
                    else this.jumpWait();

                    this.equip(...this.equipmentBeforeBot);
                }
            }
        }

        if (crashed) this.logger("Bot stopped due to a crash.");
        // C# restarts the script with auto-relogin on (Bot.Manager.RestartScriptAsync): API gap.
        else if (!bot.player.loggedIn) this.logger("Bot stopped due to player logout.");
        else this.logger("Bot stopped successfully.");

        return this.scriptFinished;
    }

    /** The part of StopBot C# runs in a background task (`StopBotAsync`); Butler log: dropped. */
    private stopBotBackground(): void {
        this.savedState(false);

        if (this.antiLag) {
            bot.options.setFps = 30; // tato keep this fps 30 ples
            if (bot.flash.get<boolean>("ui.monsterIcon.redX.visible") === true) bot.flash.callGameFunction("world.toggleMonsters");
        }

        bot.options.customName = bot.player.username || this.username();
        const guild = bot.flash.get<string | null>("world.myAvatar.objData.guild.Name");
        bot.options.customGuild = guild ? `< ${guild} >` : "";
    }

    /** C# `StopBotEvent`: the `scriptStopping` handler (`error` is set when the script failed). */
    stopBotEvent(error: string | null): boolean {
        this.removeStopBotEvent?.();
        this.removeStopBotEvent = null;
        this.setOptions(false);
        return this.stopBot(error !== null);
    }

    // C# CrashDetector: crash reports, dropped.
    private joinedPrison = false;
    private prisonListenerActive = false;

    // ---------------------------------------------------------------------------------------
    // Inventory
    // ---------------------------------------------------------------------------------------

    /**
     * Check the Bank, Inventory, House and Temp Inventory for the item (name or ID).
     * Unbanks it when found in the bank (unless `toInv` is false). `null` counts as owned.
     * @param item Name or ID of the item.
     * @param quant Desired quantity.
     * @param toInv Whether or not send the item to Inventory.
     */
    checkInventory(item: string | number | null | undefined, quant?: number, toInv?: boolean): boolean;
    /**
     * Check if the Bank/Inventory has all (or, with `any`, at least one) of the listed items.
     * @param items Names or IDs of the items.
     * @param quant Desired quantity.
     * @param any If any of the items exist, returns true.
     * @param toInv Whether or not send the items to Inventory.
     */
    checkInventory(items: (string | number)[] | null | undefined, quant?: number, any?: boolean, toInv?: boolean): boolean;
    checkInventory(
        item: string | number | (string | number)[] | null | undefined,
        quant = 1,
        anyOrToInv?: boolean,
        toInvArg?: boolean,
    ): boolean {
        if (Array.isArray(item)) {
            const any = anyOrToInv ?? false;
            const toInv = toInvArg ?? true;
            if (item.length === 0) return true;
            for (const name of item) {
                if (this.checkInventory(name, quant, toInv)) {
                    if (any) return true;
                    continue;
                }
                if (!any) return false;
            }
            return !any;
        }

        const toInv = anyOrToInv ?? true;
        if (item === null || item === undefined) return true;

        if (bot.tempInv.contains(item, quant)) return true;
        if (bot.inventory.contains(item, quant)) return true;
        if (bot.house.contains(item)) return true;

        if (bot.bank.contains(item)) {
            if (toInv) this.unbank(item);
            if (
                (toInv && bot.inventory.getQuantity(item) >= quant) ||
                (!toInv && (bot.bank.getItem(item)?.quantity ?? 0) >= quant)
            )
                return true;
        }
        return false;
    }

    /**
     * Calls `initializer` until it returns a value (not null/undefined/false/0), up to `retries`
     * times with `delay` ms between tries; null when every try failed.
     */
    initializeWithRetries<T>(initializer: () => T, retries = 5, delay = 1000): T | null {
        for (let i = 0; i < retries; i++) {
            try {
                const result = initializer();
                if (result !== null && result !== undefined && (result as unknown) !== false && (result as unknown) !== 0)
                    return result;
            } catch (error) {
                rethrowIfStopped(error);
                this.logger(`Attempt ${i + 1}/${retries} threw exception: ${errorMessage(error)}`);
            }
            this.logger(`Attempt ${i + 1}/${retries}: Initialization failed. Retrying...`);
            this.sleep(delay);
        }
        this.logger(`Initialization failed after ${retries} attempts at: ${initializer.name || "initializer"}.`);
        return null;
    }

    /**
     * Moves items (names or IDs) from the bank to the inventory or house, skipping items that are
     * already there or nowhere. Retries each move up to 20 times.
     */
    unbank(...items: (string | number)[]): void {
        const names = items.filter((x): x is string => typeof x === "string");
        const ids = items.filter((x): x is number => typeof x === "number");
        if (names.length > 0) this.unbankNames(names);
        if (ids.length > 0) this.unbankIds(ids);
    }

    private unbankNames(items: string[]): void {
        if (items.length === 0) return;
        if (bot.player.inCombat) this.jumpWait();

        let requiredSpaces = items.length;
        for (const item of items) {
            const inHouse = bot.house.contains(item);
            const inInventory = bot.inventory.contains(item);
            const inBank = bot.bank.contains(item);
            if (inHouse || inInventory || (!inHouse && !inInventory && !inBank)) {
                requiredSpaces--;
                continue;
            }

            if (inBank && (!inInventory || !inHouse)) {
                // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
                if (this.usesCategoryInventory()) {
                    const categoryItem = bot.bank.items.find((c) => equalsIgnoreCase(c.name, item));
                    if (!categoryItem) continue;

                    if (!this.hasSpaceFor(categoryItem)) {
                        this.logger(`The ${this.inventoryPoolFor(categoryItem)} inventory is full. Attempting to make room.`);
                        this.bankInventoryPoolFor(categoryItem);
                        if (!this.hasSpaceFor(categoryItem)) {
                            this.logger(`The ${this.inventoryPoolFor(categoryItem)} inventory is still full. Please clear space and restart.`, "", {
                                messageBox: true,
                                stopBot: true,
                            });
                            return;
                        }
                    }
                } else {
                    if (this.inventoryBagFreeSlots <= 0 && bot.inventory.slots !== 0 && bot.inventory.usedSlots >= bot.inventory.slots) {
                        this.logger(
                            `⚠️ Your inventory is full (${bot.inventory.usedSlots}/${bot.inventory.slots}. Attempting to make room by banking Misc Ac items`,
                        );
                        // Try *All* banking metods to try and make space.
                        this.bankACMisc();
                        this.bankACUnenhancedGear();
                        this.bankACHouseItems();
                    }

                    // Retry after banking misc stuff
                    if (this.inventoryBagFreeSlots <= 0 && bot.inventory.slots !== 0 && bot.inventory.usedSlots >= bot.inventory.slots) {
                        if (this.inventoryBagFreeSlots <= 0)
                            this.logger(
                                `⚠️ Your inventory is full (${bot.inventory.usedSlots}/${bot.inventory.slots}) — please make ${requiredSpaces} space(s) and restart the bot.`,
                                "",
                                { messageBox: true, stopBot: true },
                            );
                        else return;
                    }
                }

                const bankItem = bot.bank.getItem(item);
                const isHouseItem = !!bankItem && this.categoryStrings.includes(bankItem.category);
                let success = false;

                if (isHouseItem) {
                    if (!bankItem) {
                        this.logger(`❌ Failed to get bank item for '${item}', skipping.`);
                        continue;
                    }
                    for (let i = 0; i < 20; i++) {
                        this.sendPackets(`%xt%zm%bankToInv%${bot.map.roomId}%${bankItem.id}%${bankItem.charItemId}%`);
                        this.sleep();
                        if (bot.house.contains(item)) {
                            success = true;
                            break;
                        }
                    }
                } else {
                    for (let i = 0; i < 20; i++) {
                        bot.bank.ensureToInventory(item);
                        this.sleep();
                        if (bot.inventory.contains(item)) {
                            success = true;
                            break;
                        }
                    }
                }

                if (!success) {
                    this.logger(`🚫 Failed to unbank ${item}, skipping it.`);
                    continue;
                }
                this.logger(`✅ ${item} successfully moved from bank! 🏦➡️🎒`);
            }
        }
    }

    private unbankIds(itemIds: number[]): void {
        if (itemIds.length === 0 || !itemIds.some((id) => id !== 0)) return;
        if (bot.player.inCombat) this.jumpWait();

        let requiredSpaces = itemIds.length;
        for (const itemId of itemIds) {
            const inHouse = bot.house.contains(itemId);
            const inInventory = bot.inventory.contains(itemId);
            const inBank = bot.bank.contains(itemId);
            if (inHouse || inInventory || (!inHouse && !inInventory && !inBank)) {
                requiredSpaces--;
                continue;
            }

            if (inBank && (!inInventory || !inHouse)) {
                const bankItem = bot.bank.items.find((x) => x.id === itemId);
                if (!bankItem) {
                    this.logger(`❌ Failed to find item with ID ${itemId}, skipping it.`);
                    continue;
                }

                if (
                    // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
                    this.usesCategoryInventory()
                        ? !this.hasSpaceFor(bankItem)
                        : this.inventoryBagFreeSlots <= 0 && bot.inventory.slots !== 0 && bot.inventory.usedSlots >= bot.inventory.slots
                ) {
                    this.logger("The destination inventory is full. Please clear space and restart the bot.", "", {
                        messageBox: true,
                        stopBot: true,
                    });
                    return;
                }

                const isHouseItem = this.categoryStrings.includes(bankItem.category);
                let success = false;
                if (isHouseItem) {
                    for (let i = 0; i < 20; i++) {
                        this.sendPackets(`%xt%zm%bankToInv%${bot.map.roomId}%${bankItem.id}%${bankItem.charItemId}%`);
                        this.sleep();
                        if (bot.house.contains(itemId)) {
                            success = true;
                            break;
                        }
                    }
                } else {
                    for (let i = 0; i < 20; i++) {
                        bot.bank.ensureToInventory(itemId);
                        this.sleep();
                        if (bot.inventory.contains(itemId)) {
                            success = true;
                            break;
                        }
                    }
                }

                if (!success) {
                    this.logger(`🚫 Failed to unbank ${bankItem.name}, skipping it.`);
                    continue;
                }
                this.logger(`✅ ${bankItem.name} successfully moved from bank! 🏦➡️🎒`);
            }
        }
    }

    /** Item IDs never banked (Treasure Potion, Dark Potion). */
    private static readonly extras = new Set([18927, 38575]);

    /**
     * Moves items (names or IDs) from the inventory/house to the bank. Skips classes, equipped,
     * blacklisted and configured-gear items.
     */
    toBank(...items: (string | number)[]): void {
        const names = items.filter((x): x is string => typeof x === "string");
        const ids = items.filter((x): x is number => typeof x === "number");
        if (names.length > 0) this.toBankNames(names);
        if (ids.length > 0) this.toBankIds(ids);
    }

    private static readonly bankWhiteList = ["note", "item", "resource", "questitem"];

    private toBankNames(items: string[]): void {
        if (!items.some((name) => !!name)) return;

        for (const item of items) {
            if (
                !item ||
                item === this.farmClass ||
                item === this.soloClass ||
                item === this.dodgeClass ||
                item === this.bossClass ||
                this.farmGear.includes(item) ||
                this.soloGear.includes(item) ||
                this.dodgeGear.includes(item) ||
                this.bossGear.includes(item)
            )
                continue;

            if (bot.inventory.isEquipped(item) || bot.house.isEquipped(item)) {
                this.logger(`⚔️ Can't bank equipped item: ${item}`);
                continue;
            }

            if (bot.bank.contains(item)) {
                this.logger(`ℹ️ ${item} is already in bank, skipping.`);
                continue;
            }

            const inventoryItem = [...bot.inventory.items, ...bot.house.items].find((x) => x.name === item);
            if (!inventoryItem) {
                this.debugLogger(`❌ ${item} not found in inventory, skipping.`, "ToBank Debug");
                continue;
            }

            // TEMPORARY Game4000 class-banking guard. Game4000 classes cannot be banked, including AC-tagged classes.
            if (categoryIs(inventoryItem, "Class")) {
                this.logger(`Skipping class banking for item ID ${inventoryItem.id}. Classes cannot be banked in Game4000.`);
                continue;
            }

            // Coin (AC) items have infinite bank space, only block non-coin items when bank is full
            if (!inventoryItem.coins && bot.bank.freeSlots <= 0) {
                this.logger(`🏦 Bank is full, skipping ${item}.`);
                continue;
            }

            if (
                (!CoreBots.bankWhiteList.some((c) => categoryIs(inventoryItem, c)) && !inventoryItem.coins) ||
                this.bankingBlackList.includes(item) ||
                CoreBots.extras.has(inventoryItem.id)
            ) {
                this.logger(`⛔ ${item} is blacklisted or excluded.`);
                continue;
            }

            const houseItem = bot.house.getItem(item);
            const itemIsForHouse = !!houseItem && this.categoryStrings.includes(houseItem.category);

            if (!itemIsForHouse) {
                this.jumpWait();
                let success = false;
                for (let i = 0; i < 5; i++) {
                    if (bot.inventory.ensureToBank(item)) {
                        success = true;
                        break;
                    }
                    this.sleep();
                }
                if (!success && !bot.bank.contains(item)) {
                    this.logger(`🚫 Failed to bank ${item} after 5 attempts, skipping.`);
                    continue;
                }
            } else {
                if (!houseItem) continue;
                if (bot.house.freeSlots <= 0) {
                    this.logger(`🏠 House storage full, skipping ${item}.`);
                    continue;
                }
                this.jumpWait();
                this.sendPackets(`%xt%zm%bankFromInv%${bot.map.roomId}%${houseItem.id}%${houseItem.charItemId}%`);
                this.waitForTrue(() => !bot.house.contains(item), 20);
                if (bot.house.items.some((x) => x.name === item)) {
                    this.logger(`🚫 Failed to bank ${item} from house storage, skipping.`);
                    continue;
                }
            }
            this.logger(`💰 ${item} moved to bank successfully! 🎒➡️🏦`);
        }
    }

    private toBankIds(items: number[]): void {
        if (!items.some((id) => id > 0)) return;

        for (const itemId of items) {
            if (itemId <= 0 || CoreBots.extras.has(itemId) || bot.inventory.isEquipped(itemId) || bot.house.isEquipped(itemId)) continue;

            const inventoryItem = [...bot.inventory.items, ...bot.house.items].find((x) => x.id === itemId);
            if (!inventoryItem) continue;
            const label = inventoryItem.name || `ID: ${itemId}`;

            // TEMPORARY Game4000 class-banking guard.
            if (categoryIs(inventoryItem, "Class")) {
                this.logger(`Skipping class banking for item ID ${inventoryItem.id}. Classes cannot be banked in Game4000.`);
                continue;
            }

            // Coin (AC) items have infinite bank space, only block non-coin items when bank is full
            if (!inventoryItem.coins && bot.bank.freeSlots <= 0) {
                this.logger(`🏦 Bank is full, skipping ${label}.`);
                continue;
            }

            if (
                (!CoreBots.bankWhiteList.some((c) => categoryIs(inventoryItem, c)) && !inventoryItem.coins) ||
                this.bankingBlackList.includes(inventoryItem.name)
            ) {
                this.logger(`⛔ ${label} is blacklisted or excluded.`);
                continue;
            }

            const itemIsForHouse = bot.house.items.some((x) => x.id === itemId && this.categoryStrings.includes(x.category));

            if (!itemIsForHouse) {
                this.jumpWait();
                let success = false;
                for (let attempt = 0; attempt < 20; attempt++) {
                    bot.inventory.ensureToBank(itemId);
                    if (bot.bank.contains(itemId)) {
                        success = true;
                        break;
                    }
                    if (attempt === 10) this.logger(`⏳ Still trying to bank ${label}, attempt ${attempt + 1}/20...`);
                    this.sleep();
                }
                if (success) this.logger(`💰 ${label} moved to bank! 🎒➡️🏦`);
                else this.logger(`🚫 Failed to bank ${label} after 20 attempts.`);
            } else {
                const houseItem = bot.house.items.find((x) => x.id === itemId);
                if (!houseItem) continue;
                if (bot.house.freeSlots <= 0) {
                    this.logger(`🏠 House storage full, skipping ${label}.`);
                    continue;
                }
                this.jumpWait();
                this.sendPackets(`%xt%zm%bankFromInv%${bot.map.roomId}%${houseItem.id}%${houseItem.charItemId}%`);
                this.waitForTrue(() => !bot.house.contains(itemId), 20);
                if (bot.house.items.some((x) => x.id === itemId)) this.logger(`🚫 Failed to bank ${label} from house storage.`);
                else this.logger(`🏠💰 ${label} moved to bank from house storage! 🎒➡️🏦`);
            }
        }
    }

    /** Moves non-equipped house items (names or IDs) to the house bank. */
    toHouseBank(...items: (string | number)[]): void {
        if (!items.some((x) => (typeof x === "string" ? x.length > 0 : x > 0))) return;
        this.jumpWait();

        for (const item of items) {
            if (typeof item === "string") {
                if (bot.house.freeSlots <= 0 || !item || item === this.soloClass || item === this.farmClass) continue;
                const itemExists = bot.house.items.some((x) => x.name === item && (!x.coins || !x.equipped));
                if (!itemExists) continue;
                if (bot.house.contains(item)) {
                    if (!bot.house.ensureToBank(item)) {
                        this.logger(`🚫 Failed to bank ${item}, skipping it.`);
                        continue;
                    }
                    this.logger(`🏠💰 ${item} moved to house bank successfully!`);
                }
            } else {
                if (item === 0 || bot.house.freeSlots <= 0) continue;
                const itemExists = bot.house.items.some((x) => x.id === item && !x.equipped && x.coins);
                if (!itemExists) continue;
                if (bot.house.contains(item)) {
                    if (!bot.house.ensureToBank(item)) {
                        this.logger(`🚫 Failed to bank ${item}, skipping it.`);
                        continue;
                    }
                    this.logger(`🏠💰 Item ID ${item} moved to house bank successfully!`);
                }
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Shops
    // ---------------------------------------------------------------------------------------

    /** Item categories that go to the house inventory. */
    categoryStrings: string[] = ["House", "Wall Item", "Floor Item"];

    /**
     * Buys an item (name or ID) till you have the desired quantity.
     * @param map Map of the shop.
     * @param shopId ID of the shop.
     * @param item Name or ID of the item.
     * @param quant Desired quantity.
     * @param shopItemId For merge shops with the same item more than once (Skua's loader shows it).
     * @param index Which of several matching shop entries to buy.
     * @param log Log the purchase.
     */
    buyItem(map: string, shopId: number, item: string | number, quant = 1, shopItemId = 0, index = 0, log = true): void {
        if (typeof item === "number" && this.checkInventory(item, quant)) return;

        this._checkInventorySpace();
        const shopItem = this.parseShopItem(this.getShopItems(map, shopId), shopId, item, shopItemId);
        if (!shopItem) {
            this.logger(
                typeof item === "string"
                    ? `Failed to find the item '${item}' in the shop with ID ${shopId}, skipping it.`
                    : `Failed to find the item with ID ${item} in the shop with ID ${shopId}, skipping it`,
            );
            return;
        }
        if (shopItem.category && this.categoryStrings.includes(shopItem.category)) this._checkHouseSpace();
        this._buyItem(map, shopId, shopItem, quant, index, log);
    }

    private buyRetries = 0;

    /** Buys `item` from the shop (after the checks in {@link buyItem}). */
    _buyItem(map: string, shopId: number, item: CoreShopItem | null, quant: number, index = 0, log = true): void {
        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;
        bot.options.attackWithoutTarget = false;

        const staticQuant = quant;
        if (!item) return;

        if (bot.map.name !== map) {
            this.join(map);
            bot.wait.forMapLoad(map);
        }

        while (!bot.shouldExit && bot.player.inCombat) {
            if (bot.player.hasTarget) bot.combat.cancelTarget();
            this.jumpWait();
            this.sleep();
        }

        let retry = 0;
        while (!bot.shouldExit && bot.shops.id !== shopId) {
            bot.shops.load(shopId);
            bot.wait.forActionCooldown("loadShop");
            this.waitForTrue(() => bot.shops.isLoaded && bot.shops.id === shopId, 20);
            this.sleep(1000);
            if (bot.shops.id === shopId) {
                this.logger(`Shop loaded: "${bot.flash.get<string>("world.shopinfo.sName")}" (${shopId})`, "Loading Shop");
                break;
            }
            if (retry === 20) break;
            retry++;
        }

        // Resolve the correct ShopItem by index when multiple matches exist
        const itemId = item.id;
        const matches = this.readShopItems().filter((x) => x.id === itemId);
        const resolvedItem = matches.length > index ? matches[index] : matches[0];
        if (!resolvedItem) {
            this.logger(`Failed to find ${item.name} at index ${index} in shop ${shopId}.`, "BuyItem");
            return;
        }
        item = resolvedItem;
        const shopItem = item;

        const buyQuant = this._calcBuyQuantity(shopItem, quant);
        if (buyQuant <= 0 || !this.canBuy(map, shopId, shopItem, quant)) return;

        let succeeded = false;
        for (let i = 0; i < 5; i++) {
            if (this.getShopItemData(shopItem.id, shopItem.shopItemId, false)) {
                succeeded = true;
                break;
            }
            this.sleep(1000);
        }
        if (!succeeded) {
            this.logger(`Failed to find the shopItemData for itemID ${shopItem.id} in ${shopId}${CoreBots.reinstallCleanFlash}`, "BuyItem");
            this.logger("BuyItem Failed, crashed 5 times", "", { stopBot: true });
            return;
        }

        const offRelog = bot.events.on("extensionPacket", (packet) => {
            const type = packet?.params?.type;
            const data = packet?.params?.dataObj;
            if (type !== "json") return;
            switch (data?.strMessage) {
                case "Item is not buyable. Item Inventory full. Re-login to syncronize your real bag slot amount.":
                    this.relogin("Inventory de-sync (AE Issue) detected, relogging so the bot can continue");
                    // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
                    if (this.usesCategoryInventory() ? !this.hasSpaceFor(shopItem, buyQuant) : this.inventoryBagFreeSlots < 1)
                        this.logger("The destination inventory has insufficient space after login. Please clear space.", "", { stopBot: true });
                    break;
                case "Quest Complete Failed: Missing Required Item":
                    this.relogin("Quest de-sync (AE Issue) detected, relogging so the bot can continue");
                    break;
            }
        });
        try {
            this.sleep(1000);
            bot.wait.forActionCooldown("buyItem");
            // C# calls world.sendBuyItemRequestWithQuantity({iSel: item, iQty, accept: 1}); skua.swf's
            // buyItemByID builds the same request (turu can't pass objects to game functions).
            bot.flash.call("buyItemByID", shopItem.id, shopItem.shopItemId || -1, buyQuant);
            this.waitForTrue(() => bot.inventory.contains(shopItem.id, quant) || bot.bank.contains(shopItem.id, quant), 20);
            this.sleep();
        } finally {
            offRelog();
        }

        const shown = buyQuant === 302500 ? 1 : buyQuant;
        if (this.checkInventory(shopItem.id, staticQuant)) {
            if (log) this.logger(`Bought ${shown} ${shopItem.name}`, "BuyItem");
        } else if (this.buyRetries < 5) {
            this.logger(`Failed at buying ${shown} ${shopItem.name}, retrying: x${this.buyRetries}`, "BuyItem");
            this.buyRetries++;
            this.jumpWait();
            this._buyItem(map, shopId, shopItem, quant, index, log);
        } else {
            this.buyRetries = 0;
            this.logger(`Failed at buying ${shown} ${shopItem.name}`, "BuyItem");
        }
    }

    /** C# `_BuyItem._canBuy`: achievement, membership, required item, quest, reputation, merge and cost checks. */
    private canBuy(map: string, shopId: number, item: CoreShopItem | null, buyQuant: number): boolean {
        if (!item) return false;

        // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
        if (this.usesCategoryInventory()) {
            const actualQuantity = this._calcBuyQuantity(item, buyQuant);
            if (!this.hasSpaceFor(item, actualQuantity)) {
                this.bankInventoryPoolFor(item);
                if (!this.hasSpaceFor(item, actualQuantity)) return false;
            }
        } else if (!this.hasSpace && !this.checkInventory(item.id, 1, false)) {
            // Attempt to bank something
            this.bankACMisc(1);
            // Recheck for space
            if (!this.hasSpace) return false;
        }

        // Achievement Check
        const achievementId = num(bot.flash.get("world.shopinfo.iIndex"));
        const io = bot.flash.get<string | null>("world.shopinfo.sField");
        if (achievementId > 0 && io && !this.hasAchievement(achievementId, io)) {
            this.logger(`Cannot buy ${item.name} from ${shopId} because you dont have achievement ${achievementId} of category ${io}.`, "CanBuy");
            return false;
        }

        // Member Check
        if (item.upgrade && !bot.player.isMember) {
            this.logger(`Cannot buy ${item.name} from ${shopId} because you aren't a member.`, "CanBuy");
            return false;
        }

        // Required-Item Check
        const reqItemId = num(bot.flash.get("world.shopinfo.reqItems"));
        if (reqItemId > 0 && !this.checkInventory(reqItemId)) {
            this.logger(
                `Cannot buy ${item.name} from ${shopId} because you dont have the requiered item needed to buy stuff from the shop, itemID: ${reqItemId}`,
                "CanBuy",
            );
            return false;
        }

        // Quest Check: C# looks the shop item's sQuest up in Skua's QuestData.json to name the
        // quests to complete. That file doesn't exist in Turu; the purchase is attempted and fails
        // in-game (logged by the retry loop) if the quest is missing.

        // Rep check
        if (item.faction && item.faction !== "None") {
            const reqRank = this.pointsToLevel(item.requiredReputation);
            if (reqRank > this.factionRank(item.faction)) {
                this.logger(`Cannot buy ${item.name} from ${shopId} because you dont have rank ${reqRank} ${item.faction}.`, "CanBuy");
                return false;
            }
        }

        // Merge item check
        const itemCount = item.quantity === 0 || item.quantity === 302500 ? 1 : item.quantity;
        const buyCount = Math.ceil(buyQuant / itemCount);
        for (const req of item.requirements) {
            if (this.checkInventory(req.id, req.quantity)) continue;

            bot.drops.pickup(req.id);
            bot.wait.forPickup(req.id);

            const totalQuant = buyCount * req.quantity;
            if (this.getShopItems(map, shopId).some((x) => req.id === x.id)) this.buyItem(map, shopId, req.id, totalQuant, 0, 0, true);

            if (!this.checkInventory(req.id, totalQuant)) {
                if (this.checkInventory(req.id)) {
                    this.logger(
                        `Cannot buy ${req.name} from ${shopId}.\nYou own ${bot.inventory.getQuantity(req.id)}x ${req.name}.\nYou need ${totalQuant}.`,
                        "CanBuy",
                    );
                    return false;
                }
                this.logger(`Cannot buy ${item.name} from ${shopId} because ${req.name} is missing.`, "CanBuy");
                return false;
            }
        }

        if (item.cost > 0) {
            if (!item.coins) {
                // Gold check
                const totalGoldCost = buyCount * item.cost;
                if (totalGoldCost > 100000000) {
                    this.logger("Cannot buy more than 100 mil worth of items.", "CanBuy");
                    return false;
                } else if (totalGoldCost > bot.player.gold) {
                    this.logger(`Cannot buy ${item.name} from ${shopId}.`, "CanBuy");
                    this.logger(`You own ${bot.inventory.getQuantity(item.id)}x ${item.name}.`, "CanBuy");
                    this.logger(`You need ${bot.inventory.getQuantity(item.id) + buyCount}.`, "CanBuy");
                    this.logger(`You are missing ${totalGoldCost - bot.player.gold} gold to buy enough.`, "CanBuy");
                    return false;
                }
            } else {
                // AC costing check. C# asks with a Yes/No message box; Turu has none, so it
                // answers "No" (never spends AC without the user's say).
                const totalAcCost = buyCount * item.cost;
                this.logger(
                    `The bot is about to buy "${item.name}" ${buyCount} times, which costs ${totalAcCost} AC. ` +
                        "Turu can't ask for confirmation, so the purchase is skipped.",
                    "CanBuy",
                );
                this.logger(`Cannot buy ${item.name} from ${shopId} because you didn't allow the bot to buy the item`, "CanBuy");
                return false;
            }
        }
        return true;
    }

    /**
     * Determines the maximum quantity of an item that can be purchased from a specified shop.
     * API gap: C# asks the game (`world.maximumShopBuys(item)`), which takes an object argument
     * that turu can't pass. This returns the space left in the stack instead.
     */
    maxBuyQuant(map: string, shopId: number, item: CoreShopItem | null): number {
        if (!item) return 0;

        this.join(map);
        bot.wait.forMapLoad(map);

        // Wait until out of combat, cancelling any targets
        while (!bot.shouldExit && bot.player.inCombat) {
            if (bot.player.hasTarget) bot.combat.cancelTarget();
            this.jumpWait();
            this.sleep();
        }

        // Load the specified shop with retries
        let retry = 0;
        while (!bot.shouldExit && bot.shops.id !== shopId && retry < 20) {
            bot.shops.load(shopId);
            bot.wait.forActionCooldown("loadShop");
            this.waitForTrue(() => bot.shops.isLoaded && bot.shops.id === shopId, 20);
            this.sleep(1000);
            retry++;
        }
        if (bot.shops.id !== shopId) {
            this.logger(`Failed to load shop ${shopId} after ${retry} attempts.`, "MaxBuyQuant");
            return 0;
        }

        let data: Raw | null = null;
        for (let i = 0; i < 5; i++) {
            data = this.initializeWithRetries(() => this.getShopItemData(item.id, item.shopItemId));
            if (data) break;
            this.sleep(1000);
        }
        if (!data) {
            this.logger(`Failed to load shop item data for ItemID ${item.id} in ShopID ${shopId}.`, "MaxBuyQuant");
            return 0;
        }

        this.sleep(1000); // Prevent potential race conditions
        bot.wait.forActionCooldown("buyItem");
        const owned = this.allItems().filter((x) => x.id === item.id).reduce((sum, x) => sum + x.quantity, 0);
        return Math.max(0, item.maxStack - owned);
    }

    /** The raw `world.shopinfo.items` entry of an item (shopItemId 0 = any). */
    private getShopItemData(itemId: number, shopItemId = 0, log = true): Raw | null {
        const shopItems = bot.flash.get<Raw[] | null>("world.shopinfo.items");
        if (!shopItems) {
            if (log) this.logger("Shop items data could not be retrieved.", "GetShopItemData");
            return null;
        }
        const found = shopItems.find((i) => i && num(i.ItemID) === itemId && (shopItemId === 0 || num(i.ShopItemID) === shopItemId));
        if (found) return found;
        if (log) this.logger(`Shop item data not found for ItemID ${itemId}, ShopItemID ${shopItemId}.`, "GetShopItemData");
        return null;
    }

    private _checkInventorySpace(): void {
        // Game4000 purchases are checked in canBuy using the selected item.
        // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
        if (this.usesCategoryInventory()) return;

        if (bot.inventory.slots !== 0 && this.inventoryBagFreeSlots <= 0) {
            const usedBefore = bot.inventory.usedSlots;
            this.logger(
                `Your inventory is full [${usedBefore}/${bot.inventory.slots}]. Attempting to bank AC-tagged misc items...`,
                "_CheckInventorySpace",
            );
            this.bankACMisc();
            const freed = usedBefore - bot.inventory.usedSlots;
            const s = freed !== 1 ? "s" : "";
            if (this.inventoryBagFreeSlots <= 0)
                this.logger(
                    `Banked ${freed} item${s} but your inventory is still full. Please clear space manually. Stopping the bot.`,
                    "_CheckInventorySpace",
                    { stopBot: true },
                );
            else {
                const free = this.inventoryBagFreeSlots;
                this.logger(`Banked ${freed} item${s}. ${free} slot${free !== 1 ? "s" : ""} now available.`, "_CheckInventorySpace");
            }
        }
    }

    private _checkHouseSpace(): void {
        if (bot.house.slots !== 0 && bot.house.freeSlots <= 0) {
            const usedBefore = bot.house.usedSlots;
            this.logger(`Your house inventory is full [${usedBefore}/${bot.house.slots}]. Attempting to bank AC-tagged misc items...`, "_CheckHouseSpace");
            this.bankACHouseItems();
            const freed = usedBefore - bot.house.usedSlots;
            const s = freed !== 1 ? "s" : "";
            if (bot.house.freeSlots <= 0)
                this.logger(`Banked ${freed} item${s} but your house is still full. Please clear space manually. Stopping the bot.`, "_CheckHouseSpace", {
                    stopBot: true,
                });
            else {
                const free = bot.house.freeSlots;
                this.logger(`Banked ${freed} item${s}. ${free} slot${free !== 1 ? "s" : ""} now available.`, "_CheckHouseSpace");
            }
        }
    }

    /** Inventory, bank, house and temp inventory items. */
    private allItems(): InventoryItem[] {
        return [...bot.inventory.items, ...bot.bank.items, ...bot.house.items, ...bot.tempInv.items];
    }

    /** How many to buy (in whole bundles) to reach `requestedAmount`, capped by the max stack; 0 = don't buy. */
    _calcBuyQuantity(item: CoreShopItem, requestedAmount: number): number {
        if (requestedAmount > item.maxStack) {
            this.logger(`Requested ${requestedAmount}, but max stack for ${item.name} is ${item.maxStack}. Fix the calling script.`, "BuyItem");
            bot.stop();
        }

        if (bot.bank.contains(item.id) && !bot.inventory.contains(item.id)) this.unbank(item.id);

        const itemStackSize = item.quantity > 0 ? item.quantity : 1;
        const all = this.allItems();
        const currentStock = all.filter((x) => x.id === item.id).reduce((sum, x) => sum + x.quantity, 0);
        const stacksNeeded = Math.ceil(requestedAmount / itemStackSize);

        for (const req of item.requirements) {
            const totalNeeded = stacksNeeded * req.quantity;
            const reqCurrent = all.filter((x) => x.id === req.id).reduce((sum, x) => sum + x.quantity, 0);
            if (reqCurrent < totalNeeded) {
                this.logger(`Missing ${req.name} (${reqCurrent}/${totalNeeded}). Cannot proceed with purchase.`);
                return 0;
            }
        }

        let buyAmount = stacksNeeded * itemStackSize;
        const maxCanBuy = item.maxStack - currentStock;
        buyAmount = Math.min(buyAmount, maxCanBuy - (maxCanBuy % itemStackSize));

        if (buyAmount <= 0) {
            this.logger(`Cannot buy more ${item.name}, max stack reached (${currentStock}/${item.maxStack}).`);
            return 0;
        }
        return buyAmount;
    }

    private static readonly repThresholds: [rank: number, repRequired: number][] = [
        [1, 0],
        [2, 900],
        [3, 3600],
        [4, 10000],
        [5, 22500],
        [6, 44100],
        [7, 78400],
        [8, 129600],
        [9, 202500],
        [10, 302500],
    ];

    /** Reputation points to rank. */
    pointsToLevel(points: number): number {
        for (const [rank, req] of CoreBots.repThresholds) if (points <= req) return rank;
        return 10; // max rank if somehow above cap
    }

    /**
     * Sells an item (name or ID) till you have the desired quantity.
     * @param item Name or ID of the item.
     * @param quant How many to sell.
     * @param all Set to true if you wish to sell all the items.
     */
    sellItem(item: string | number, quant = 1, all = false): void {
        let invItem: InventoryItem | null;
        let itemName: string;
        if (typeof item === "string") {
            itemName = item;
            invItem = bot.inventory.getItem(itemName);
            if (!(quant > 0 ? this.checkInventory(itemName, quant) : this.checkInventory(itemName)) || !invItem) return;
        } else {
            const found = [...bot.inventory.items, ...bot.bank.items].find((x) => x.id === item);
            if (!found) {
                this.logger(`Item with ID ${item} not found.`);
                return;
            }
            itemName = found.name;
            if (!(quant > 0 ? this.checkInventory(itemName, quant) : this.checkInventory(itemName))) return;
            invItem = null;
        }

        const label = typeof item === "string" ? itemName : `Item ID ${item}`;
        let found: InventoryItem | undefined;
        for (let i = 0; i < 5; i++) {
            found = [...bot.inventory.items, ...bot.bank.items].find((x) =>
                typeof item === "string" ? x.name === itemName : x.id === item,
            );
            if (found) break;
            this.logger(`Attempt ${i + 1}: ${typeof item === "string" ? `Item ${itemName}` : `Item ID ${item}`} not found. Retrying...`);
            this.sleep(1000);
        }
        if (typeof item === "number" && !found) {
            this.logger(`Item ID ${item} not found after 5 attempts.`);
            return;
        }

        if (bot.bank.contains(itemName) && !bot.inventory.contains(itemName)) this.unbank(itemName);

        const sellId = typeof item === "string" ? invItem!.id : item;
        const charItemId = typeof item === "string" ? invItem!.charItemId : found!.charItemId;
        const suffix = typeof item === "string" ? "" : ` (ID: ${item})`;

        let retryCount = 0;
        const sellCount = all ? bot.inventory.getQuantity(itemName) : quant;
        const quantAfterSale = bot.inventory.getQuantity(itemName) - sellCount;
        for (;;) {
            this.jumpWait();
            bot.wait.forActionCooldown("sellItem");
            bot.send.packet(`%xt%zm%sellItem%${bot.map.roomId}%${sellId}%${sellCount}%${charItemId}%`);
            bot.wait.forItemSell();
            this.sleep();

            if (!all && (bot.inventory.contains(itemName) || bot.inventory.getQuantity(itemName) === quantAfterSale)) {
                if (bot.inventory.getQuantity(itemName) === quantAfterSale) this.logger(`Sold x${sellCount} "${itemName}"${suffix}`);
                return;
            } else if (all && !this.checkInventory(itemName)) {
                this.logger(`Sold all of "${itemName}"${suffix}`);
                return;
            } else if (retryCount < 5) {
                retryCount++;
                this.logger(`${label} failed to sell, retrying [Try x${retryCount}]`);
            } else {
                this.logger(`${label} failed to sell, retrying x${retryCount} times did not succeed`);
                return;
            }
        }
    }

    /** The loaded shop's items (raw `world.shopinfo.items`, falling back to `bot.shops.items`). */
    private readShopItems(): CoreShopItem[] {
        const raw = bot.flash.get<Raw[] | null>("world.shopinfo.items");
        if (Array.isArray(raw)) return raw.filter((x) => x).map(toShopItem);
        return bot.shops.items.map((x) => ({
            id: x.id,
            shopItemId: x.shopItemId,
            name: x.name,
            quantity: x.quantity,
            maxStack: x.maxStack,
            category: x.category,
            upgrade: x.upgrade,
            coins: x.coins,
            cost: x.cost,
            faction: "",
            requiredReputation: 0,
            requirements: [],
        }));
    }

    /**
     * Retrieves the items of a shop (joins its map and loads it first).
     * @returns The shop's items, or an empty list if the shop could not be loaded.
     */
    getShopItems(map: string, shopId: number): CoreShopItem[] {
        if (!equalsIgnoreCase(bot.map.name, map)) {
            this.join(map);
            bot.wait.forMapLoad(map);
        }

        let retry = 0;
        while (!bot.shouldExit && retry++ < 20) {
            if (bot.shops.isLoaded && bot.shops.id === shopId) break;
            bot.shops.load(shopId);
            bot.wait.forActionCooldown("loadShop");
            this.sleep(1000);
        }

        if (!bot.shops.isLoaded || bot.shops.id !== shopId) {
            this.logger(`Failed to load shop ${shopId} in map ${map}.`);
            return [];
        }

        const items = this.readShopItems();
        if (items.length === 0) {
            this.logger(`Shop ${shopId} loaded but contained no items.`);
            return [];
        }
        return items;
    }

    /**
     * Picks a shop item by ShopItemID (when > 0) then by name (exactly one match) or item ID.
     * @returns The item, or null (logged) if it isn't there or the name is ambiguous.
     */
    parseShopItem(shopItems: CoreShopItem[], shopId: number, item: string | number, shopItemId = 0): CoreShopItem | null {
        if (shopItems.length === 0) {
            this.logger(`Shop ${shopId} has no items loaded.`);
            return null;
        }

        if (shopItemId > 0) {
            const bySid = shopItems.find((x) => x.shopItemId === shopItemId);
            if (bySid) return bySid;
            this.logger(
                typeof item === "string"
                    ? `Item with ShopItemID ${shopItemId} not found in shop ${shopId}. Using "${item}" string as backup method`
                    : `ShopItemID ${shopItemId} not found in shop ${shopId}. Falling back to ItemID...`,
            );
        }

        if (typeof item === "number") {
            const byItemId = shopItems.find((x) => x.id === item);
            if (byItemId) return byItemId;
            this.logger(`Item ID ${item} not found in shop ${shopId}.`);
            return null;
        }

        const matches = shopItems.filter((x) => equalsIgnoreCase(x.name, item));
        if (matches.length === 0) {
            this.logger(`Item '${item}' not found in shop ${shopId}.`);
            return null;
        }
        if (matches.length > 1) {
            this.logger(`Multiple items found with the name '${item}' in shop ${shopId}. Specify ShopItemID.`);
            return null;
        }
        return matches[0];
    }

    // C# GhostItem: only used by Join's ghost-item map bypasses, which this port leaves out.

    // ---------------------------------------------------------------------------------------
    // Drops
    // ---------------------------------------------------------------------------------------

    /** Adds drops (names or IDs) to the pickup list and un-banks them. */
    addDrop(...items: (string | number)[]): void {
        if (items.length === 0) return;
        const names = items.filter((x): x is string => typeof x === "string");
        const ids = items.filter((x): x is number => typeof x === "number");
        if (names.length > 0) {
            this.unbank(...names);
            for (const item of names) bot.drops.add(item);
        }
        if (ids.length > 0) {
            this.unbank(...ids);
            bot.drops.add(...ids);
        }
    }

    /** Removes drops (names or IDs) from the pickup list. */
    removeDrop(...items: (string | number)[]): void {
        if (items.length === 0) return;
        const toPickup = bot.drops.toPickup;
        const toPickupIds = bot.drops.toPickupIds;
        const names = unique(items.filter((x): x is string => typeof x === "string" && x.trim().length > 0).map((x) => x.trim())).filter((x) =>
            toPickup.includes(x),
        );
        const ids = unique(items.filter((x): x is number => typeof x === "number")).filter((x) => toPickupIds.includes(x));
        if (names.length > 0) bot.drops.remove(...names);
        if (ids.length > 0) bot.drops.remove(...ids);
    }

    // ---------------------------------------------------------------------------------------
    // Registered quests
    // ---------------------------------------------------------------------------------------

    private registeredQuests: number[] = [];
    private questLoop: QuestLoop | null = null;
    private readonly lastTurnInSeen = new Map<number, number>();
    private turnInEventOff: (() => void) | null = null;
    /** Whether WE believe the quest log is open (a heuristic, see C#). */
    private questLogAssumedOpen = false;

    private markTurnInSeen(questId: number): void {
        this.lastTurnInSeen.set(questId, Date.now());
    }

    /** ms since the last turn-in seen for the quest (0 when none). */
    private timeSinceLastTurnIn(questId: number): number {
        const last = this.lastTurnInSeen.get(questId);
        return last === undefined ? 0 : Date.now() - last;
    }

    private ensureTurnInEventHooked(): void {
        if (this.turnInEventOff) return;
        this.turnInEventOff = bot.events.on("questTurnedIn", ({ questId }) => {
            this.debugLogger(`[QuestTurnIn] Turn-in event fired (QuestID=${questId}).`, "RegisterQuest Debug");
            this.markTurnInSeen(questId);
        });
    }

    private ensureQuestLogOpen(): void {
        if (!this.questLogAssumedOpen) {
            this.logger("[QuestTurnIn] Quest log believed closed — opening it.");
            bot.flash.callGameFunction("world.toggleQuestLog");
        } else {
            this.logger("[QuestTurnIn] Quest log believed open — cycling it (close, wait, reopen).");
            bot.flash.callGameFunction("world.toggleQuestLog"); // close
            this.pause(2500);
            bot.flash.callGameFunction("world.toggleQuestLog"); // reopen
        }
        this.questLogAssumedOpen = true;
    }

    /**
     * Registers quests to be accepted and turned in automatically while the bot does something
     * else (C# runs three background tasks; here they're Core handlers polled from Core's waits):
     * - a watcher that turns quests in as soon as they're completable,
     * - a manager that accepts quests that aren't in progress,
     * - a watchdog that nudges the quest log when a completable quest hasn't turned in for ~60 s.
     *
     * Previously registered quests are cancelled first. Member-only quests (for non-members) and
     * quests with missing accept requirements are skipped with a log line.
     */
    registerQuests(...questIds: number[]): void {
        if (questIds.length === 0) return;

        this.cancelRegisteredQuests();
        this.ensureTurnInEventHooked();

        const chooseQuests = new Map<number, QuestData>();
        const nonChooseQuests = new Map<number, QuestData>();

        for (const questId of unique(questIds)) {
            const q = this.initializeWithRetries(() => this.ensureLoad(questId));
            if (!q) {
                this.logger(`Failed to initialize quest with ID ${questId}.`);
                continue;
            }

            if (q.upgrade && !bot.player.isMember) {
                this.logger(`Quest ${questId} requires membership, but the player is not a member.`);
                continue;
            }

            const missingRequirements = q.acceptRequirements.filter((x) => x && !this.checkInventory(x.id));
            if (missingRequirements.length > 0) {
                this.logger(
                    `Player is missing the following accept requirements for quest ${questId}: ${missingRequirements.map((x) => x.name).join(", ")}`,
                );
                continue;
            }

            if (q.simpleRewards.some((r) => r.type === 2)) chooseQuests.set(q.id, q);
            else nonChooseQuests.set(q.id, q);

            const requiredItems = [...q.acceptRequirements.filter((x) => !x.temp), ...q.requirements.filter((x) => !x.temp)].filter(
                (item) => item && item.id > 0,
            );
            for (const item of requiredItems) {
                if (!bot.drops.toPickup.includes(item.name) || !bot.drops.toPickupIds.includes(item.id)) {
                    this.addDrop(item.name);
                    this.addDrop(item.id);
                }
            }

            this.unbank(...unique([...q.acceptRequirements, ...q.requirements].map((x) => x.id)));
        }

        const allQuests = [...chooseQuests.values(), ...nonChooseQuests.values()];
        this.registeredQuests = allQuests.map((q) => q.id);

        this.lastTurnInSeen.clear();
        for (const quest of allQuests) this.markTurnInSeen(quest.id);

        if (allQuests.length === 0) this.logger("No quests left to register (all skipped, e.g. member-only or not available).");

        const loop: QuestLoop = {
            quests: allQuests,
            chooseQuests: new Set(chooseQuests.keys()),
            inFlight: new Map(),
            stuckCounts: new Map(),
            threshold: (60 + randomInt(2, 6)) * 1000,
        };
        this.questLoop = loop;

        this.registerHandler(QUEST_WATCHER, Math.min(this.actionDelay, 500), () => this.questWatcher(loop));
        this.registerHandler(QUEST_MANAGER, Math.max(this.actionDelay, 100) + allQuests.length * this.actionDelay * 2, () =>
            this.questManager(loop),
        );
        this.registerHandler(QUEST_WATCHDOG, 2000, () => this.questWatchdog(loop));
    }

    /** Fast watcher: turns quests in as soon as they become completable. */
    private questWatcher(loop: QuestLoop): void {
        try {
            for (const quest of loop.quests) {
                if (!bot.player.alive) continue;

                const checkAt = loop.inFlight.get(quest.id);
                if (checkAt !== undefined) {
                    // C# awaits ActionDelay * 2 after the turn-in, then checks whether it's stuck.
                    if (Date.now() >= checkAt) {
                        loop.inFlight.delete(quest.id);
                        this.checkStuckQuest(loop, quest);
                    }
                    continue;
                }

                if (bot.quests.isInProgress(quest.id) && bot.quests.canComplete(quest.id)) {
                    this.completeRegisteredQuest(loop, quest);
                    loop.inFlight.set(quest.id, Date.now() + this.actionDelay * 2);
                }
            }
        } catch (error) {
            rethrowIfStopped(error);
            this.logger(`[QuestTurnIn] Watcher failed during an iteration; retrying: ${errorMessage(error)}`);
        }
    }

    private completeRegisteredQuest(loop: QuestLoop, quest: QuestData): void {
        let rewardId = -1;
        if (loop.chooseQuests.has(quest.id) && bot.quests.active.some((aq) => aq.id === quest.id)) {
            const reward = this.questData(quest.id)?.rewards.find((r) => r.quantity < r.maxStack);
            rewardId = reward?.id ?? -1;
        }
        const turnIns = quest.once || quest.field ? 1 : num(bot.flash.callGameFunction("world.maximumQuestTurnIns", quest.id));
        bot.send.packet(`%xt%zm%tryQuestComplete%${bot.map.roomId}%${quest.id}%${rewardId}%false%${turnIns}%wvz%`);
    }

    private checkStuckQuest(loop: QuestLoop, quest: QuestData): void {
        let stuckCount = loop.stuckCounts.get(quest.id) ?? 0;
        if (bot.quests.isInProgress(quest.id)) stuckCount++;
        else stuckCount = 0;
        loop.stuckCounts.set(quest.id, stuckCount);

        if (stuckCount >= 20 && bot.quests.isInProgress(quest.id)) {
            this.pause(this.actionDelay * 2);
            bot.flash.callGameFunction("world.abandonQuest", quest.id);
            this.pause(this.actionDelay * 2);
            bot.quests.load(quest.id);
            this.pause(this.actionDelay * 2);
            // Keep this recovery Accept().
            bot.quests.accept(quest.id);
            loop.stuckCounts.set(quest.id, 0);
        }
        // Don't re-accept here: the manager accepts the quest once it's no longer active.
    }

    /** Slower manager: accepts quests that aren't currently in progress. */
    private questManager(loop: QuestLoop): void {
        try {
            for (const quest of loop.quests) {
                if (!bot.player.alive) continue;
                if (loop.inFlight.has(quest.id)) continue;
                if (!bot.quests.ensureLoad(quest.id)) continue;
                if (!bot.quests.isInProgress(quest.id)) bot.quests.accept(quest.id);
            }
        } catch (error) {
            rethrowIfStopped(error);
            this.logger(`[QuestTurnIn] Quest manager failed during an iteration; retrying: ${errorMessage(error)}`);
        }
    }

    /** Turn-in watchdog: nudges the quest log when a completable quest hasn't turned in for ~60 s. */
    private questWatchdog(loop: QuestLoop): void {
        try {
            const stuckCompletable = loop.quests
                .filter((q) => bot.quests.isInProgress(q.id) && bot.quests.canComplete(q.id))
                .filter((q) => this.timeSinceLastTurnIn(q.id) >= loop.threshold);
            if (stuckCompletable.length === 0) return;

            const stuckQuest = stuckCompletable[0];
            const elapsed = this.timeSinceLastTurnIn(stuckQuest.id);
            this.logger(
                `Quest ${stuckQuest.id} (${stuckQuest.name}) has been completable without a turn-in for ` +
                    `${Math.round(elapsed / 1000)}s (threshold ${Math.round(loop.threshold / 1000)}s) — nudging quest log.`,
            );
            this.ensureQuestLogOpen();
            for (const quest of stuckCompletable) this.markTurnInSeen(quest.id);
            loop.threshold = (60 + randomInt(2, 6)) * 1000;
        } catch (error) {
            rethrowIfStopped(error);
            this.logger(`[QuestTurnIn] Watchdog failed during an iteration; retrying: ${errorMessage(error)}`);
        }
    }

    /** Cancels the current registered quests. */
    cancelRegisteredQuests(): void {
        bot.lite.reacceptQuest = false;

        if (this.questLoop) {
            this.removeHandler(QUEST_WATCHER);
            this.removeHandler(QUEST_MANAGER);
            this.removeHandler(QUEST_WATCHDOG);
            this.questLoop = null;
        }

        if (this.turnInEventOff) {
            this.turnInEventOff();
            this.turnInEventOff = null;
        }

        this.questLogAssumedOpen = false;
        this.lastTurnInSeen.clear();

        // C#: only when the engine-level registry (Bot.Quests.RegisterQuests) is in use.
        if (bot.quests.registered.length > 0) {
            bot.quests.unregisterQuests(...this.registeredQuests);
            this.abandonQuest(...this.registeredQuests);
        }
        this.registeredQuests = [];
    }

    // ---------------------------------------------------------------------------------------
    // Quests
    // ---------------------------------------------------------------------------------------

    /**
     * Ensures the quest is accepted: membership check, unbanks the required items and adds
     * them to the drop list, then accepts it.
     * @param questId ID of the quest to accept.
     */
    ensureAccept(questId = 0): boolean {
        const questData = this.initializeWithRetries(() => this.ensureLoad(questId));
        if (!questData) {
            this.logger(`Failed to load quest with ID ${questId} after multiple attempts.`);
            return false;
        }

        if (questData.upgrade && !bot.player.isMember) this.logger(`"${questData.name}" [${questId}] is member-only, stopping the bot.`, "", { stopBot: true });

        if (questId <= 0) return false;

        const requiredItems = [...questData.acceptRequirements.filter((x) => !x.temp), ...questData.requirements.filter((x) => !x.temp)].filter(
            (item) => item && item.id > 0,
        );
        // Add both ID and Name to the drop list if missing (ID is incase of duplicate names)
        for (const item of requiredItems) {
            if (!bot.drops.toPickup.includes(item.name) || !bot.drops.toPickupIds.includes(item.id)) {
                this.addDrop(item.name);
                this.addDrop(item.id);
            }
        }

        this.sleep(this.actionDelay * 2);
        if (bot.quests.isInProgress(questId)) return true;
        bot.wait.forActionCooldown("acceptQuest");
        bot.quests.ensureAccept(questId);
        bot.wait.forQuestAccept(questId);
        return true;
    }

    /**
     * Completes the quest, with a choose-able reward item.
     * @param questId ID of the quest to complete.
     * @param itemId ID of the choose-able reward item (-1 picks the first reward you don't have maxed).
     */
    ensureComplete(questId: number, itemId = -1): boolean {
        if (questId <= 0) return false;

        const questData = this.initializeWithRetries(() => this.ensureLoad(questId));
        if (!questData) {
            this.logger(`Failed to load quest with ID ${questId} after multiple attempts.`);
            return false;
        }

        if (!bot.drops.toPickupIds.includes(itemId) && itemId > 0) bot.drops.add(itemId);

        if (!bot.quests.isInProgress(questId)) this.ensureAccept(questId);

        if (
            questData.requirements.length === 0 ||
            (questData.requirements.every((r) => r && r.id > 0) &&
                this.checkInventory(questData.requirements.map((x) => x.id)) &&
                this.checkInventory(questData.acceptRequirements.map((x) => x.id)))
        ) {
            if (itemId === -1 && questData.simpleRewards.length > 0) {
                const availableReward = questData.simpleRewards.find((x) => x.id > 0 && !this.checkInventory(x.id, x.maxStack, false));
                itemId = availableReward?.id ?? -1;
                if (availableReward && itemId !== -1 && !bot.drops.toPickupIds.includes(itemId)) this.addDrop(availableReward.id);
                return bot.quests.ensureComplete(questId, itemId);
            }
            return bot.quests.ensureComplete(questId, itemId);
        }
        return false;
    }

    // C# EnsureComplete(params int[]): not used by the ported scripts, dropped.

    // ---- TEMPORARY Game4000 inventory compatibility helpers ----

    /** Ordinary bag entries, excluding Game4000 misc items and classes. */
    get inventoryBagUsedSlots(): number {
        return this.openInventoryCategories()?.countBag() ?? bot.inventory.usedSlots;
    }

    get inventoryBagFreeSlots(): number {
        return bot.inventory.slots - this.inventoryBagUsedSlots;
    }

    /**
     * Checks the item's destination pool, existing stack, and merge/token space.
     * `quantity` is the actual purchase amount, not the desired final total.
     */
    hasSpaceFor(item: InventoryItem | CoreShopItem | null | undefined, quantity = 1): boolean {
        if (!item) return false;
        const categories = this.openInventoryCategories();
        if (!categories) return this.inventoryBagFreeSlots > 0 || bot.inventory.contains(item.id);

        const data = this.inventorySpaceItemData(item);
        if (GameInventoryCategories.isHouse(data)) return bot.house.freeSlots > 0;

        const freedSlots = "shopItemId" in item ? categories.freedBy(data, quantity) : 0;
        return !categories.isFullFor(data, bot.inventory.slots, freedSlots);
    }

    private usesCategoryInventory(): boolean {
        const result = bot.flash.callGameFunction<unknown>("loaderInfo.applicationDomain.hasDefinition", "InvCat");
        if (result === null || result === undefined || result === "") throw new Error("Cannot determine the game inventory categories.");
        return flag(result);
    }

    private openInventoryCategories(): GameInventoryCategories | null {
        return this.usesCategoryInventory() ? new GameInventoryCategories() : null;
    }

    /** Raw item data (keeps fields such as bHouse, TokID and TokQty that the API omits). */
    private inventorySpaceItemData(item: InventoryItem | CoreShopItem, rawItems?: Raw[] | null): Raw {
        const isShop = "shopItemId" in item;
        const items = rawItems ?? bot.flash.get<Raw[] | null>(isShop ? "world.shopinfo.items" : "world.bankinfo.items");
        const found = items?.find((c) => c && num(c.ItemID) === item.id && (!isShop || num(c.ShopItemID) === item.shopItemId));
        return (
            found ?? {
                ItemID: item.id,
                sName: item.name,
                iQty: item.quantity,
                iStk: item.maxStack,
                sType: item.category,
                bCoins: item.coins,
                bUpg: item.upgrade,
                sMeta: "meta" in item ? item.meta : null,
            }
        );
    }

    private inventoryPoolFor(item: InventoryItem | CoreShopItem): string {
        return this.openInventoryCategories()?.poolOf(this.inventorySpaceItemData(item)) ?? "bag";
    }

    private filterInventoryPool(items: InventoryItem[], pool: string | null): InventoryItem[] {
        if (pool === null || items.length === 0) return items;
        const categories = this.openInventoryCategories();
        if (!categories) return items;
        const rawItems = bot.flash.get<Raw[] | null>("world.myAvatar.items");
        return items.filter((item) => categories.poolOf(this.inventorySpaceItemData(item, rawItems)) === pool);
    }

    private bankInventoryPoolFor(item: InventoryItem | CoreShopItem, requiredSpaces = 1): void {
        switch (this.inventoryPoolFor(item)) {
            case "misc":
                this.bankACMisc(requiredSpaces, "misc");
                break;
            case "bag":
                this.bankACUnenhancedGear(requiredSpaces, "bag");
                break;
            case "house":
                this.bankACHouseItems();
                break;
        }
    }

    /** Whether an ordinary bag entry can fit. Use {@link hasSpaceFor} for a specific item. */
    get hasSpace(): boolean {
        return this.inventoryBagFreeSlots > 0;
    }

    /**
     * Turns a quest in as many times as possible (or `amount` times), with a choose-able reward.
     * @param questId ID of the quest.
     * @param amount How many turn-ins, -1 is the maximum possible.
     * @param itemId ID of the choose-able reward item.
     * @returns How many turn-ins were sent (0 when it failed).
     */
    ensureCompleteMulti(questId: number, amount = -1, itemId = -1): number {
        const quest = this.initializeWithRetries(() => this.ensureLoad(questId));
        if (!quest) {
            this.logger(`Quest ${questId} not loaded after 5 attempts.`);
            return 0;
        }
        if (itemId > 0) bot.drops.add(itemId);

        if (!bot.quests.isInProgress(questId)) this.ensureAccept(questId);
        this.waitForTrue(() => bot.quests.isInProgress(questId), 20);

        let turnIns: number;
        const requiredItemNames = [...quest.requirements, ...quest.acceptRequirements].map((item) => item.name);

        if (quest.once || quest.field) turnIns = 1;
        else {
            const possibleTurnin = num(bot.flash.callGameFunction("world.maximumQuestTurnIns", questId));
            turnIns = possibleTurnin > amount && amount > 0 ? amount : possibleTurnin;
            if (turnIns === 0) return 0;
        }

        // Ensure quest is loaded, and is entirely completable.
        if (this.ensureAccept(questId) && this.checkInventory(requiredItemNames))
            bot.flash.callGameFunction("world.tryQuestComplete", questId, itemId, false, turnIns);

        bot.wait.forQuestComplete(questId);
        bot.wait.forQuestAccept(questId);

        return !bot.quests.isInProgress(questId) ? turnIns : 0;
    }

    /**
     * The quest from the client's quest tree with its reward details, or null if it isn't loaded.
     * (turu's `Quest` has no reward max stacks or reward types; they're read from `world.questTree`.)
     */
    questData(questId: number): QuestData | null {
        const quest = bot.quests.get(questId);
        return quest ? this.withRewardDetails(quest) : null;
    }

    /** `bot.quests.ensureLoad` returning {@link QuestData} (C# `Bot.Quests.EnsureLoad`). */
    loadQuest(questId: number): QuestData | null {
        const quest = bot.quests.ensureLoad(questId);
        return quest ? this.withRewardDetails(quest) : null;
    }

    private withRewardDetails(quest: Quest): QuestData {
        const path = `world.questTree.${quest.id}`;
        const oRewards = bot.flash.get<Record<string, Record<string, Raw>> | null>(`${path}.oRewards`);
        const reward = bot.flash.get<Raw[] | null>(`${path}.reward`);

        const rewards: RewardItem[] = [];
        if (oRewards && typeof oRewards === "object")
            for (const group of Object.values(oRewards))
                if (group && typeof group === "object") for (const item of Object.values(group)) if (item) rewards.push(toRewardItem(item));

        const simpleRewards: SimpleReward[] = Array.isArray(reward)
            ? reward.filter((r) => r).map((r) => ({ ...toRewardItem(r), rate: num(r.iRate), type: num(r.iType) }))
            : [];

        // Fall back to turu's lists when the raw data isn't there.
        const fromApi = (items: QuestItem[]): RewardItem[] => items.map((x) => ({ ...x, maxStack: 0 }));
        return {
            ...quest,
            rewards: rewards.length > 0 ? rewards : fromApi(quest.rewards),
            simpleRewards: simpleRewards.length > 0 ? simpleRewards : fromApi(quest.simpleRewards).map((x) => ({ ...x, rate: 0, type: 0 })),
        };
    }

    /** An empty quest (C# `new Quest()`), returned when loading fails. */
    private static emptyQuest(): QuestData {
        return {
            id: 0,
            name: "",
            status: null,
            once: false,
            upgrade: false,
            level: 0,
            requirements: [],
            acceptRequirements: [],
            rewards: [],
            simpleRewards: [],
            slot: 0,
            value: 0,
            field: "",
            index: 0,
        };
    }

    /**
     * Loads a quest into the client (if needed) and returns it. Stops the bot if it can't be
     * loaded. (C# also falls back to Skua's QuestData.json file, which Turu doesn't have.)
     */
    ensureLoad(questId: number): QuestData {
        if (questId <= 0) {
            this.logger(`❌ Invalid QuestID: ${questId}`, "EnsureLoad");
            return CoreBots.emptyQuest(); // safe empty quest
        }

        let quest = this.questData(questId);
        if (!quest) {
            this.sleep();
            for (let i = 0; i < 20 && !bot.quests.get(questId); i++) {
                bot.quests.load(questId);
                this.pause(100);
            }
            quest = this.questData(questId);
        }

        if (!quest) {
            this.logger(`Failed to get the Quest Object for questID ${questId}${CoreBots.reinstallCleanFlash}`, "EnsureLoad A.0", {
                messageBox: true,
                stopBot: true,
            });
            return CoreBots.emptyQuest();
        }
        return quest;
    }

    /** Loads several quests (in batches of 30) and returns them; stops the bot if any is missing. */
    ensureLoadMany(...questIds: number[]): QuestData[] {
        if (questIds.length === 0) return [];

        const loaded = () => questIds.map((id) => this.questData(id)).filter((q): q is QuestData => q !== null);
        const quests = loaded();
        if (quests.length === questIds.length) return quests;

        const missing = questIds.filter((id) => !quests.some((q) => q.id === id));
        this.sleep();
        const batchSize = 30;
        for (let i = 0; i < missing.length; i += batchSize) {
            bot.quests.load(...missing.slice(i, i + batchSize));
            this.sleep(1500);
        }

        this.waitForTrue(() => questIds.every((id) => bot.quests.get(id) !== null), 20);

        const toReturn = loaded();
        if (toReturn.length < questIds.length) {
            this.logger(`Failed to get the Quest Object for questIDs ${questIds.join(" | ")}${CoreBots.reinstallCleanFlash}`, "EnsureLoad B.4", {
                messageBox: true,
                stopBot: true,
            });
            return [];
        }
        return toReturn;
    }

    /** Abandons the quests that are active. */
    abandonQuest(...questIds: number[]): void {
        if (questIds.length === 0) return;
        for (const q of this.ensureLoadMany(...questIds)) {
            if (!q || q.status === null) continue;
            bot.flash.callGameFunction("world.abandonQuest", q.id);
            this.waitForTrue(() => this.ensureLoad(q.id).status === null, 20);
            bot.quests.unregisterQuests(q.id);
        }
    }

    /** The reward names of the quests (for {@link addDrop}). */
    questRewards(...questIds: number[]): string[] {
        if (questIds.length === 0) return [];
        const quests = this.initializeWithRetries(() => this.ensureLoadMany(...questIds));
        if (!quests) {
            this.logger(`Failed to load quests with IDs: ${questIds.join(", ")}`, "QuestRewards");
            return [];
        }
        return quests.flatMap((q) => q.rewards.filter((r) => r).map((r) => r.name));
    }

    // C# QuestRewards' <= 15-quests branch reads the same rewards from Bot.Quests.Tree.

    /** Accepts and then completes the quest, used inside a loop. */
    chainComplete(questId: number, itemId = -1): void {
        if (itemId > 0) bot.drops.add(itemId);
        this.initializeWithRetries(() => this.ensureLoad(questId));
        this.ensureCompleteMulti(questId, -1, itemId);
    }

    /** Checks if a quest has been completed before (its quest-chain value). */
    isCompletedBefore(questId: number, log = true): boolean {
        if (questId <= 0) return false;

        let quest = this.initializeWithRetries(() => this.ensureLoad(questId));
        if (!quest) {
            this.logger(`❌ Failed to initialize quest ${questId} after multiple attempts.`);
            return false;
        }

        const questName = quest.name || `${questId}`;
        const checkCompletion = (questData: QuestData | null): boolean => {
            if (!questData) {
                this.logger(`❌ Quest data for ${questName} [${questId}] is null.`);
                return false;
            }
            const complete = questData.slot < 0 || num(bot.flash.callGameFunction("world.getQuestValue", questData.slot)) >= questData.value;
            if (log) this.logger(`${questName} [${questId}] completion check [${complete ? "✔" : "❌"}]`);
            return complete;
        };

        try {
            return checkCompletion(quest);
        } catch (error) {
            rethrowIfStopped(error);
            quest = this.initializeWithRetries(() => this.ensureLoad(questId));
            if (!quest) {
                this.logger(`❌ Failed to reinitialize ${questName} [${questId}] after multiple attempts.`);
                return false;
            }
            return checkCompletion(quest);
        }
    }

    // ---------------------------------------------------------------------------------------
    // Combat
    // ---------------------------------------------------------------------------------------

    /**
     * Joins a map, jumps to the cell, sets the spawn point and kills the monster (once, or until
     * you have `quant` of `item`).
     * @param monster Name of the monster to kill ("*" = any).
     * @param item Item to kill the monster for, if null will just kill the monster 1 time.
     * @param quant Desired quantity of the item.
     */
    killMonster(map: string, cell: string, pad: string, monster: string, item: string | null = null, quant = 1, options: KillOptions = {}): void {
        const { isTemp = true, log = true, publicRoom = false } = options;
        if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) return;

        if (bot.map.name !== map) {
            this.join(map, "Enter", "Spawn", { publicRoom });
            bot.wait.forMapLoad(map);
            this.waitForTrue(() => bot.player.loaded, 10);
        }

        const cells = bot.map.cells;
        if (!cells.some((c) => equalsIgnoreCase(c, cell))) cell = cells.find((c) => equalsIgnoreCase(c, cell)) ?? cell;

        pad = toTitleCase(pad);

        if (bot.player.cell !== cell) {
            bot.map.jump(cell, pad, this.inPublicRoom());
            bot.wait.forCellChange(cell);
        }

        bot.player.setSpawnPoint();

        if (item !== null && !isTemp) this.addDrop(item);

        bot.options.aggroAllMonsters = false;

        const targetMonster = this.findMonster(map, monster);
        if (!targetMonster) {
            this.logger(`⚠️ Monster ${monster} not found in cell ${cell}, pad ${pad} in /${map}`);
            return;
        }

        if (item === null) {
            while (!bot.shouldExit) {
                if (!bot.player.alive) this.waitForTrue(() => bot.player.alive, 20);

                if (bot.player.cell !== cell) {
                    bot.map.jump(cell, pad, this.inPublicRoom());
                    bot.wait.forCellChange(cell);
                }

                this.canWeAggro();

                // C#: `!HasTarget || Target != null || Target?.HP > 0`.
                const target: Monster | null = bot.player.target;
                if (!bot.player.hasTarget || target !== null) bot.combat.attack(monster);

                this.sleep(500);
                // Check if player doenst have a target after the attacking; if not then its dead and we can move on
                if (!bot.player.hasTarget) return;
            }
        } else {
            this._killForItem(monster, item, quant, isTemp, false, log, cell);
        }

        bot.options.attackWithoutTarget = false;
        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;

        const mapCells = bot.map.cells;
        const monsterCells = new Set(bot.monsters.mapMonsters.map((m) => m.cell));
        const enterCount = mapCells.filter((c) => c.includes("Enter")).length;
        const targetCell =
            mapCells
                .filter((c) => c && !this.blackListedJumptoCells.includes(c) && !monsterCells.has(c))
                .find((c) => c && (enterCount > 1 || !c.includes("Enter"))) ?? "Enter";

        bot.map.jump(targetCell, targetCell === "Enter" ? "Spawn" : "Left", this.inPublicRoom());
        bot.wait.forCellChange(targetCell);
        this.sleep();
        this.jumpWait();
        this.rest();
        bot.options.hidePlayers = false;
    }

    // C# KillMonster(map, cell, pad, int MonsterMapID, ...) overloads: not used by the ported scripts, dropped.

    /**
     * Joins a map and hunts for the monster (jumping to its cell).
     * @param item Item to hunt the monster for. If null, will just hunt and kill the monster once.
     * @param quant Desired quantity of the item.
     */
    huntMonster(map: string, monster: string, item: string | null = null, quant = 1, options: KillOptions = {}): void {
        const { isTemp = true, log = true, publicRoom = false } = options;
        if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) return;

        // Join the specified map
        if (bot.map.name !== map) {
            this.join(map, "Enter", "Spawn", { publicRoom });
            bot.wait.forMapLoad(map);
        }

        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;
        if (item !== null && !isTemp) this.addDrop(item);

        let targetMonster = this.findMonster(map, monster);
        if (!targetMonster) {
            this.logger(`⚠️ Monster "${monster}" not found in /${map}.`);
            return;
        }

        const username = bot.player.username;
        if (bot.map.playerNames.some((x) => x !== username)) {
            bot.options.aggroMonsters = true;
            bot.options.hidePlayers = true; // Trust Tato — reduces lag
        } else bot.options.aggroMonsters = false;

        if (item === null) {
            while (!bot.shouldExit) {
                if (!bot.player.alive) this.waitForTrue(() => bot.player.alive, 20);

                if (bot.player.cell !== targetMonster?.cell) {
                    this.jump(targetMonster?.cell ?? "Enter", targetMonster?.cell === undefined ? "Spawn" : "Left");
                    bot.wait.forCellChange(targetMonster?.cell ?? "Enter");
                    if (targetMonster?.cell !== undefined) bot.player.setSpawnPoint();
                }
                if (!bot.player.hasTarget && targetMonster && targetMonster.hp > 0) bot.combat.attack(targetMonster.name);

                this.pause(500);

                const target = bot.player.target;
                if (!bot.player.hasTarget || (target !== null && target.hp <= 0)) {
                    bot.options.attackWithoutTarget = false;
                    bot.options.aggroMonsters = false;
                    bot.options.hidePlayers = false;
                    return;
                }
            }
            this.jumpWait();
            this.rest();
        } else {
            if (log) this.logger(`Killing ${monster} for item: "${item}" ${bot.inventory.getQuantity(item)}/${quant}`, "HuntMonster");
            while (!bot.shouldExit && !(isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) {
                if (!bot.player.alive) this.waitForTrue(() => bot.player.alive, 20);

                targetMonster = this.findMonster(map, monster);
                if (!targetMonster) continue;

                if (bot.player.cell && bot.player.cell !== targetMonster.cell) {
                    const cellToJump = targetMonster.cell ?? "Enter";
                    this.jump(cellToJump, "Left");
                    bot.wait.forCellChange(cellToJump);
                }

                if (!bot.player.hasTarget && targetMonster.hp > 0) bot.combat.attack(targetMonster.name);

                this.pause(500);

                if (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant)) break;
            }

            bot.options.attackWithoutTarget = false;
            bot.options.aggroMonsters = false;
            bot.options.hidePlayers = false;
            this.toggleAggro(false);

            // Attempt to jump back to an 'Enter' or usable cell
            const cells = bot.map.cells;
            bot.map.jump(
                cells.find((c) => c.toLowerCase().includes("enter")) ??
                    cells.find((c) => !c.toLowerCase().includes("wait") && !c.toLowerCase().includes("blank") && !c.toLowerCase().includes("enter")) ??
                    "Enter",
                "Spawn",
                this.inPublicRoom(),
            );

            this.jumpWait();
            this.rest();
        }
    }

    // C# HuntMonsterQuest(questId, params (map, monster, ClassType)[]) overloads: not used, dropped.

    /**
     * Hunts the monsters for each requirement of a quest (skipping those you already have), then
     * turns it in.
     * @param mapName Map to hunt in (default: the current map).
     * @param monsterName Monster to hunt (default "*").
     */
    huntMonsterQuest(questId: number, mapName: string | null = null, monsterName: string | null = null): void {
        const quest = this.initializeWithRetries(() => this.loadQuest(questId));
        if (!quest) {
            this.logger(`⚠️ Quest ${questId} not found`);
            return;
        }

        const allRequirements = [...quest.acceptRequirements, ...quest.requirements];
        if (allRequirements.length === 0) {
            this.logger(`⚠️ Quest ${questId} has no requirements.`);
            return;
        }

        // Unbank required items.
        this.unbank(...unique(allRequirements.map((x) => x.id)));

        // Add non-temp requirements to the drop list.
        bot.drops.add(...unique(allRequirements.filter((x) => !x.temp).map((x) => x.id)));

        const huntMapName = mapName ?? bot.map.name;
        const huntMonsterName = monsterName ?? "*";

        for (const requirement of quest.requirements) {
            // Don't hunt requirements that have already been obtained.
            if (this.checkInventory(requirement.id, requirement.quantity)) continue;

            if (!bot.quests.ensureAccept(questId)) this.ensureAccept(questId);

            this.huntMonster(huntMapName, huntMonsterName, requirement.name ?? "", requirement.quantity, { isTemp: requirement.temp });
        }

        if (bot.quests.canCompleteFullCheck(questId)) this.ensureCompleteMulti(questId);
    }

    private mobFindRetry = 0;
    /** Monster name typo cache (requestedName -> correctedName). */
    private readonly monsterNameCache = new Map<string, string>();

    /**
     * Finds monsters on the map by map ID (authoritative) or name ("*" = all), with typo
     * protection for names. Stops the bot after 5 failed searches.
     */
    findMonstersList(map = "", monster = "*", monsterMapId = -1): Monster[] {
        if (!bot.player.loaded) this.waitForTrue(() => bot.player.loaded, 20);

        if (bot.map.name !== map) this.join(map);

        const candidates = bot.monsters.mapMonsters.filter((x) => x && x.name && x.name.trim().length > 0);

        // MapID is authoritative
        if (monsterMapId >= 0) {
            const byMapId = candidates.filter((x) => x.mapId === monsterMapId);
            if (byMapId.length > 0) return byMapId;
            // Fallback: some monsters use ID instead of MapID
            return candidates.filter((x) => x.id === monsterMapId);
        }

        // "*" -> everything valid
        if (!monster || monster.trim().length === 0 || monster === "*") return candidates;

        let target = formatForCompare(monster);
        // If we already auto-corrected this typo before, reuse it silently
        const corrected = this.monsterNameCache.get(target);
        if (corrected !== undefined) target = corrected;

        // Exact name match ONLY
        const matches = candidates.filter((x) => formatForCompare(x.name) === target);
        if (matches.length > 0) return matches;

        // Fuzzy fallback (typo protection)
        let closestMatch: Monster | undefined;
        let closestDistance = Number.POSITIVE_INFINITY;
        for (const candidate of candidates) {
            const distance = levenshteinDistance(formatForCompare(candidate.name), target);
            if (distance < closestDistance) {
                closestDistance = distance;
                closestMatch = candidate;
            }
        }
        if (closestMatch && closestDistance <= allowedTypoDistance(target)) {
            // cache the correction so we never log this again
            this.monsterNameCache.set(target, formatForCompare(closestMatch.name));
            this.logger(
                `⚠️ Monster "${monster}" not found in /${map}. Auto-correcting to closest match: "${closestMatch.name}" (distance ${closestDistance})`,
            );
            return [closestMatch];
        }

        // True failure log
        const visible = unique(candidates.map((x) => `"${x.name}" [${alive(x) ? "Alive" : "Dead"}]`));
        this.logger(`❌ Monster "${monster}" not found in /${map}. Visible monsters: ${visible.join(", ")}`);
        this.mobFindRetry++;

        if (this.mobFindRetry < 5) return [];

        bot.log(
            `Failed to Find the mob ${monster} within 5 retrys` +
                (monsterMapId > 0 ? `[MID: ${monsterMapId}]` : "") +
                "Do either of the following:\n" +
                "\t1. Relog, and restart the script\n" +
                "\t2. Try a different server." +
                "Script will now stop.",
        );
        return bot.stop();
    }

    /** The first living match of {@link findMonstersList} (or the first match). */
    findMonster(map: string, monster: string, monsterMapId = -1): Monster | null {
        const matches = this.findMonstersList(map, monster, monsterMapId);
        return matches.find((x) => alive(x)) ?? matches[0] ?? null;
    }

    /**
     * Kill Escherion (attacking the Staff of Inversion while he's invulnerable), once or for an item.
     * @param fromSupplies Called from Nation's Supplies (doesn't log the farming line).
     */
    killEscherion(item: string | null = null, quant = 1, options: KillOptions & { fromSupplies?: boolean } = {}): void {
        const { isTemp = false, log = true, fromSupplies = false } = options;
        if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) return;

        if (!fromSupplies && item !== null && log) this.farmingLogger(`⚔️ ${item}`, quant);

        if (item !== null && !isTemp) this.addDrop(item);

        const checkMap = (): void => {
            if (bot.map.name === "escherion" && bot.player.cell !== "Cut1" && bot.player.cell === "Boss") return;
            if (bot.map.name !== "escherion") this.join("escherion", "Boss", "Left");
            if (bot.player.cell !== "Boss") {
                bot.map.jump("Boss", "Left", this.inPublicRoom());
                bot.wait.forCellChange("Boss");
                bot.player.setSpawnPoint();
            }
            if (bot.player.cell === "Cut1") {
                bot.map.jump("Boss", "Left", this.inPublicRoom());
                bot.wait.forCellChange("Boss");
                bot.player.setSpawnPoint();
            }
        };

        const kill = (item: string | null, quant: number, isTemp: boolean): void => {
            bot.options.aggroMonsters = true;
            if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) return;

            checkMap();

            let done = false;
            while (!bot.shouldExit && !done) {
                if (!bot.player.alive) this.waitForTrue(() => bot.player.alive, 20);

                checkMap();

                // MonsterMapIDs: 2 = Staff, 3 = Escherion
                const target = bot.player.target;
                if (!bot.player.hasTarget) bot.combat.attack(3);
                else if (target?.mapId === 3 && target.state === 2 && alive(bot.monsters.mapMonsters.find((x) => x.mapId === 2)))
                    // Escherion is invulnerable -> attack Staff of Inversion
                    bot.combat.attack(2);
                else if (target?.mapId === 2 && target.hp > 0)
                    // Staff of Inversion still alive -> attack it
                    bot.combat.attack(2);
                // Otherwise, attack Escherion
                else bot.combat.attack(3);
                this.sleep();

                if (item === null && !bot.player.hasTarget) {
                    if (log) this.logger("💀 No item selected, killed Escherion once");
                    done = true;
                    break;
                } else if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) {
                    done = true;
                    break;
                }
            }

            if (!isTemp && item !== null) bot.wait.forPickup(item);
        };

        if (item === null) {
            if (log) this.logger("💀 Killing Escherion");
            kill(null, 1, false);
        } else {
            kill(item, quant, isTemp);
            this.rest();
        }
    }

    /** Kill Vath (stalagbite), attacking Stalagbite while it's up, once or for an item. */
    killVath(item: string | null = null, quant = 1, options: { isTemp?: boolean; log?: boolean } = {}): void {
        const { isTemp = false, log = true } = options;
        if (item !== null && (isTemp ? bot.tempInv.contains(item, quant) : this.checkInventory(item, quant))) return;

        this.join("stalagbite");
        bot.wait.forMapLoad("stalagbite");
        this.jump("r2", "Left");
        bot.wait.forCellChange("r2");

        const killVathInternal = (): void => {
            if (bot.map.name !== "stalagbite") {
                this.join("stalagbite");
                bot.wait.forMapLoad("stalagbite");
                this.sleep();
            }
            if (bot.player.cell !== "r2") {
                this.jump("r2");
                bot.wait.forCellChange("r2");
                this.sleep();
            }

            const monsters = bot.monsters.mapMonsters;
            const vath = monsters.find((x) => x.mapId === 7);
            const stalagbite = monsters.find((x) => x.mapId === 8);
            if (stalagbite) {
                bot.wait.forMonsterSpawn(stalagbite.name);
                if (vath) bot.combat.attack(stalagbite.state === 1 || stalagbite.state === 2 ? stalagbite.mapId : vath.mapId);
                this.sleep();
            }
        };

        if (item === null) {
            if (log) this.logger("⚡ Killing Vath");
            killVathInternal();
        } else {
            if (!isTemp) this.addDrop(item);
            if (log) this.logger(`⚡ Killing Vath for ${item} (${this.dynamicQuant(item, isTemp)}/${quant}) [Temp = ${isTemp ? "True" : "False"}]`);
            while (!bot.shouldExit && !this.checkInventory(item, quant)) killVathInternal();
        }

        bot.options.attackWithoutTarget = false;
        bot.options.aggroMonsters = false;
        bot.options.hidePlayers = false;
        this.jumpWait();
    }

    // C# _KillForItem(Monster, ...) and _KillForItem(name, int itemID, ...): not used, dropped.

    /**
     * Kills a monster (by name, "*" = any) in your cell until you have the item.
     * @param cell Cell to stay in (jumps back if moved).
     */
    _killForItem(name: string, item: string | null = null, quantity = 1, isTemp = false, rejectElse = false, log = true, cell: string | null = null): void {
        if (item === null) return;

        const trimmedName = formatForCompare(name.trim());
        const has = () => (isTemp ? bot.tempInv.contains(item, quantity) : this.checkInventory(item, quantity));
        if (has()) return;

        if (log) this.farmingLogger(item, quantity);

        while (!bot.shouldExit && !has()) {
            if (!bot.player.alive) this.waitForTrue(() => bot.player.alive, 20);

            if (cell !== null && bot.player.cell !== cell) {
                bot.map.jump(cell, "Left", this.inPublicRoom());
                bot.wait.forCellChange(cell);
            }

            this.canWeAggro();

            // Re-find monster every loop for reliability (wildcard: no target lookup)
            const target = name === "*" ? null : this.findMonster(bot.map.name, trimmedName);

            if (name === "*" && !bot.player.hasTarget) bot.combat.attack("*");
            // Only attack if we have no target, the target isn't {target} (always true here, like the
            // C# reference comparison) or it's dead.
            else if (target && (!bot.player.hasTarget || bot.player.target !== target || target.hp <= 0)) bot.combat.attack(target.name);

            if (rejectElse) bot.drops.rejectExcept(item);
            this.sleep(250); // Small delay to prevent spam
        }

        bot.wait.forPickup(item);
    }

    // C# KilledMonsters / CleanKilledMonstersList / KilledMonsterListener / RespawnListener,
    // TagsToAdd / AutoAddTags / EndGameTags: SetOptions-only bookkeeping, dropped.

    // ---------------------------------------------------------------------------------------
    // Utilities
    // ---------------------------------------------------------------------------------------

    /** The account's username (`sfc.myUserName`, else `bot.player.username`). */
    username(): string {
        try {
            const name = bot.flash.get<string | null>("sfc.myUserName");
            if (name) return name;
        } catch (error) {
            rethrowIfStopped(error);
        }
        return bot.player.username ?? "";
    }

    /**
     * Pauses for `ms` milliseconds (ActionDelay when -1, 0 or negative). Stops the script if Stop
     * was pressed. Keeps Core's handlers (registered quests, AFK, ...) running.
     */
    sleep(ms = -1): void {
        if (bot.shouldExit) bot.stop();
        // use ActionDelay if ms is -1, 0, or negative
        this.pause(ms <= 0 ? this.actionDelay : ms);
    }

    /** `bot.sleep(ms)` that keeps Core's handlers running (they're checked every 250 ms). */
    pause(ms: number): void {
        let left = ms;
        do {
            this.tick();
            const step = Math.min(Math.max(left, 0), 250);
            bot.sleep(step);
            left -= step;
        } while (left > 0);
    }

    /**
     * `bot.wait.forTrue` that keeps Core's handlers running.
     * @param timeout Tries of `interval` ms (10 = about 1 s); 0 or less waits forever.
     */
    waitForTrue(condition: () => boolean, timeout = 10, interval = 100): boolean {
        for (let i = 0; timeout <= 0 || i < timeout; i++) {
            if (condition()) return true;
            this.pause(interval);
        }
        return condition();
    }

    // ---- Handlers (C# Bot.Handlers) ----

    private handlers: Handler[] = [];
    private ticking = false;

    /**
     * Runs `run` every `intervalMs` while the script waits in Core ({@link sleep}, {@link pause},
     * {@link waitForTrue}, and so every Core kill/hunt loop). Replaces an existing handler with
     * the same name. (C# `Bot.Handlers.RegisterHandler`, which runs on its own thread.)
     */
    registerHandler(name: string, intervalMs: number, run: () => void): void {
        this.removeHandler(name);
        this.handlers.push({ name, interval: intervalMs, next: Date.now() + intervalMs, run });
    }

    removeHandler(name: string): void {
        this.handlers = this.handlers.filter((h) => h.name !== name);
    }

    clearHandlers(): void {
        this.handlers = [];
        this.questLoop = null;
    }

    /** Runs the handlers that are due. Called from Core's waits; safe to call anywhere. */
    tick(): void {
        if (this.ticking || this.handlers.length === 0) return;
        this.ticking = true;
        try {
            const now = Date.now();
            for (const handler of [...this.handlers]) {
                if (now < handler.next) continue;
                handler.next = now + handler.interval;
                try {
                    handler.run();
                } catch (error) {
                    rethrowIfStopped(error);
                    this.logger(`Handler "${handler.name}" failed: ${errorMessage(error)}`, "Handlers");
                }
            }
        } finally {
            this.ticking = false;
        }
    }

    /**
     * Logs a message to the script log (and AQW's chat when {@link loggerInChat} is on).
     * @param caller Shown in the log line (default: the calling method's name, best effort).
     */
    logger(message = "", caller = "", options: LoggerOptions = {}): void {
        const rawMessage = message ?? "";
        if (!caller) caller = callerName(1);

        bot.log(`[${timeOfDay()}] (${caller}) ${rawMessage}`);

        // AQW chat safe pipeline
        if (this.loggerInChat && bot.player.loggedIn) {
            let chatMessage = CoreBots.sanitizeAQWChat(rawMessage);
            if (chatMessage.trim().length === 0) return; // nothing meaningful left to send
            const maxLen = 180;
            if (chatMessage.length > maxLen) chatMessage = chatMessage.slice(0, maxLen);
            bot.send.clientModerator(chatMessage, caller);
        }

        // C# message box: Turu has none, the log line above is it.

        if (options.stopBot) {
            this.scriptFinished = false;
            bot.stop();
        }
    }

    private scriptFinished = true;

    private static sanitizeAQWChat(input: string): string {
        if (!input) return "";
        // Strip non-ASCII (covers box-drawing, emoji, symbols like ✓ ✦ ├ └ ═)
        input = input.replace(/[^\x20-\x7E\t\n]/g, "");
        // AQW's chat/packet layer is delimited by these
        input = input.replace(/[%<>&"']/g, "");
        input = input.replace(/[\x00\x1B\x7F\r]/g, "");
        return input.replace(/\s+/g, " ").trim();
    }

    /** Logs "Farming {item} ({have}/{quant})". */
    farmingLogger(item: string | null, quant = 1, caller = ""): void {
        const quantity = !item ? 0 : bot.tempInv.getQuantity(item) + bot.inventory.getQuantity(item);
        this.logger(`Farming ${item ?? ""} (${quantity}/${quant})`, caller || callerName(1));
    }

    /** Debug logs, only when {@link dlEnabled} (C# also resolves the script line; Turu doesn't). */
    debugLogger(marker: string | null = null, caller = ""): void {
        if (!this.dlEnabled) return;
        if (!caller) caller = callerName(1);
        if ((this.dlMarkerFilter !== null && this.dlMarkerFilter !== marker) || (this.dlCallerFilter !== null && this.dlCallerFilter !== caller)) return;
        this.logger(`${marker ?? ""}${marker ? " | " : ""}${caller}`, "DEBUG LOGGER");
    }

    dlEnabled = false;
    dlCallerFilter: string | null = null;
    dlMarkerFilter: string | null = null;

    // C# CompiledScript / Message (message boxes): no equivalent in Turu, dropped.

    /** Quantity of an item in the temp inventory, or the inventory+bank. */
    dynamicQuant(item: string, isTemp: boolean): number {
        return isTemp ? bot.tempInv.getQuantity(item) : [...bot.inventory.items, ...bot.bank.items].find((x) => x.name === item)?.quantity ?? 0;
    }

    private lastAggroStatus = false;

    /** Toggles the aggro mode off, and back on (if it was on) when enabled again. */
    toggleAggro(enable: boolean): void {
        if (enable) {
            if (this.lastAggroStatus) {
                // If was previously aggro when untoggled, set aggro back and flip last aggro
                this.lastAggroStatus = false;
                bot.options.aggroMonsters = true;
            }
        } else if (bot.options.aggroMonsters) {
            // If currently aggro, set last aggro to true and flip current aggro status
            this.lastAggroStatus = true;
            bot.options.aggroMonsters = false;
        }
    }

    /** Sends a packet `times` times (to the client with `toClient`), ActionDelay*2 apart. */
    sendPackets(packet: string, times = 1, toClient = false): void {
        for (let i = 0; i < times; i++) {
            if (toClient) bot.send.clientPacket(packet);
            else bot.send.packet(packet);
            this.sleep(this.actionDelay * 2);
        }
    }

    /** Aggro and hide players when other players are in the map, otherwise turn both off. */
    canWeAggro(): void {
        if (bot.options.aggroAllMonsters) bot.options.aggroAllMonsters = false;

        const username = bot.player.username;
        if (bot.map.playerNames.some((x) => x !== username)) {
            if (!bot.options.aggroMonsters) bot.options.aggroMonsters = true;
            if (!bot.options.hidePlayers) bot.options.hidePlayers = true; // Hide players to reduce lag
        } else {
            if (bot.options.aggroMonsters) bot.options.aggroMonsters = false;
            if (bot.options.hidePlayers) bot.options.hidePlayers = false;
        }
    }

    /** Rank of the equipped class, or of `className` from its class points (1 when not found). */
    checkClassRank(currentClass = false, className: string | null = null): number {
        if (currentClass && bot.player.currentClass !== null) {
            this.debugLogger(`Current Class: ${bot.player.currentClass} | Current Rank: ${bot.player.currentClassRank}`);
            return bot.player.currentClassRank;
        }

        const classItem = [...bot.inventory.items, ...bot.bank.items].find((i) => i.name === className && categoryIs(i, "Class"));
        if (className !== null && classItem) {
            this.logger(`Class: ${classItem.name} | Rank: ${classItem.quantity}`);
            // Minimum CP for each rank (CP < 900 stays at rank 1)
            const classPointRanks: [number, number][] = [
                [900, 2],
                [3600, 3],
                [10000, 4],
                [22500, 5],
                [44100, 6],
                [78400, 7],
                [129600, 8],
                [202500, 9],
                [302500, 10],
            ];
            let classRank = 1;
            for (const [points, rank] of classPointRanks) {
                if (classItem.quantity >= points) classRank = rank;
                else break;
            }
            this.debugLogger(`Class Rank (based on ClassXP): ${classRank}`);
            return classRank;
        }
        return 1;
    }

    /** Rests if {@link shouldRest} is on and health is below 2% of max. */
    rest(): void {
        try {
            const healthThreshold = Math.ceil(bot.player.maxHealth / 50);
            if (this.shouldRest && bot.player.alive && bot.player.health < healthThreshold) {
                bot.player.rest(false);
                this.logger(`Resting initiated: Health below threshold (${bot.player.health}/${bot.player.maxHealth}).`);
            }
        } catch (error) {
            rethrowIfStopped(error);
            this.logger(`An error occurred: ${errorMessage(error)}`);
        }
    }

    /** Logs out (C# `Bot.Servers.Logout()`, the game's `logout` function). */
    logout(): void {
        bot.flash.callGameFunction("logout");
    }

    /**
     * Relogs and goes to the house (falls back to battleon-100000).
     * API gap: turu has no `bot.servers` (C# `Bot.Servers.EnsureRelogin` with the cached login and
     * fallback servers). This logs out with auto-relogin on and waits for the engine to reconnect;
     * the script stops if the player isn't back within ~60 s.
     */
    relogin(reason = ""): void {
        bot.log(`⚡ Relogin Triggered${reason.trim() ? `: ${reason}` : ""}`);

        const origAutoRelog = bot.options.autoRelogin;
        try {
            bot.options.autoRelogin = true;
            this.logout();
            this.waitForTrue(() => !bot.player.loggedIn, 20);
            if (this.waitForTrue(() => bot.player.loggedIn && bot.player.loaded, 600)) {
                bot.log(`🏠 Relogin successful! Sending to house${reason.trim() ? `: ${reason}` : ""} ✨`);
                bot.send.packet(`%xt%zm%house%1%${this.username()}%`);
                if (!bot.wait.forMapLoad("house", 20)) {
                    bot.log("⚠️ Failed to load house map, falling back to Battleon. 🏰");
                    this.join("battleon-100000");
                }
                return;
            }
            bot.log("❌ Relogin failed after all attempts. 🛑");
            bot.stop();
        } finally {
            bot.options.autoRelogin = origAutoRelog;
        }
    }

    // C# SkuaVersionCheckerAsync / GetAtomFeedAsync / ExecuteWithRetryAsync / ComputeSHA256 /
    // ExtractLatestVersionFromAtom: auto-update, dropped.

    // ---------------------------------------------------------------------------------------
    // Classes and equipment
    // ---------------------------------------------------------------------------------------

    private currentClass: ClassType = ClassType.None;
    private usingSoloGeneric = false;
    private usingFarmGeneric = false;
    private usingDodgeGeneric = false;
    private usingBossGeneric = false;

    /**
     * Equips the configured Farm/Solo/Dodge/Boss class (and its gear) and starts its skills; with
     * no class configured, starts the skills of the class you're wearing.
     */
    equipClass(classToUse: ClassType): void {
        // Already equipped? Skip if timer is running
        if (this.currentClass === classToUse && bot.skills.timerRunning) return;
        this.currentClass = classToUse;

        const equipClass = (usingGeneric: boolean, className: string, classMode: ClassUseMode, useEquipment: boolean, equipment: string[]): boolean => {
            if (usingGeneric) {
                bot.log("Using generic");
                return false;
            }

            // Check all inventorys (inv and bank) for class
            if (!this.checkInventory(className)) {
                this.logger("You do not own " + className);
                return false;
            }

            const classItem = [...bot.inventory.items, ...bot.bank.items].find((x) => equalsIgnoreCase(x.name, className) && categoryIs(x, "Class"));
            if (!classItem) {
                this.logger(`Class "${className}" not found`);
                return false;
            }

            // If bank still contains class somehow.. unbank it
            if (bot.bank.contains(className)) {
                this.waitForTrue(() => bot.bank.ensureToInventory(classItem.id), 20);
                bot.sleep(500);
            }

            if (useEquipment && equipment.length > 0) {
                this.sleep(Math.trunc(this.actionDelay * 1.5));
                this.equip(...equipment);
            }

            const classNameLower = className.trim().toLowerCase();
            let equippedClass = bot.player.currentClass?.trim().toLowerCase();

            while (!bot.shouldExit && equippedClass !== classNameLower) {
                this.jumpWait();
                this.equip(className);
                bot.wait.forItemEquip(className);
                this.sleep(100); // small buffer to let server sync
                equippedClass = bot.player.currentClass?.trim().toLowerCase();
            }

            // Auto-enhance via registered hook (scripts that include CoreAdvanced)
            if (this.cboBool("DisableAutoEnhance") !== true) {
                for (const hook of CoreBots.onEquipClass) {
                    try {
                        hook(className);
                    } catch (error) {
                        rethrowIfStopped(error);
                        bot.log(`[Enhancement] Auto-enhance failed for ${className}: ${errorMessage(error)}`);
                    }
                }
            }

            // Start skills for the actual equipped class
            bot.skills.startAdvanced(className, false, classMode);
            return true;
        };

        const configured = (name: string) => name !== "Generic" && name !== "(Current)" && name.length > 0 && this.checkInventory(name);

        switch (classToUse) {
            case ClassType.Farm:
                if (configured(this.farmClass) && equipClass(this.usingFarmGeneric, this.farmClass, this.farmUseMode, this.farmGearOn, this.farmGear)) return;
                break;
            case ClassType.Solo:
                if (configured(this.soloClass) && equipClass(this.usingSoloGeneric, this.soloClass, this.soloUseMode, this.soloGearOn, this.soloGear)) return;
                break;
            case ClassType.Dodge:
                if (configured(this.dodgeClass) && equipClass(this.usingDodgeGeneric, this.dodgeClass, this.dodgeUseMode, this.dodgeGearOn, this.dodgeGear))
                    return;
                break;
            case ClassType.Boss:
                if (configured(this.bossClass) && equipClass(this.usingBossGeneric, this.bossClass, this.bossUseMode, this.bossGearOn, this.bossGear)) return;
                break;
            case ClassType.None:
                break;
        }

        // Fallback: start skills for current class if equip didn't run
        bot.skills.startAdvanced(bot.player.currentClass ?? "generic", false);
    }

    /** Equips items (names or IDs) that aren't equipped yet. */
    equip(...gear: (string | number)[]): void {
        for (const item of gear) {
            if (typeof item === "string" ? item.trim().length === 0 : item <= 0) continue;
            if (bot.inventory.isEquipped(item)) continue;

            if (!this.checkInventory(item)) {
                if (!bot.shouldExit) this.logger(`Equipping Failed: "${item}" not found in Inventory or Bank`);
                continue;
            }
            const invItem = bot.inventory.getItem(item);
            if (!invItem) {
                if (!bot.shouldExit) this.logger(`Equipping Failed: Could not parse "${item}" from your inventory`);
                continue;
            }
            this._equip(invItem);
        }
    }

    private _equip(item: InventoryItem | null): void {
        if (!item) {
            this.logger(`Equipping Failed: Parsed object for "${item}" is null`);
            return;
        }

        // Exit combat mode
        while (!bot.shouldExit && bot.player.inCombat) {
            if (bot.player.hasTarget) bot.combat.cancelTarget();
            this.jumpWait();
            this.sleep();
        }

        // Attempt to equip the item up to 5 times
        for (let attempt = 0; attempt < 5; attempt++) {
            if (bot.inventory.items.some((x) => x.id === item.id && x.equipped)) break;
            if (bot.player.inCombat) this.jumpWait();

            if (item.category.toLowerCase() === "item") {
                // Consumables: C# builds the item object for the game's toggleItemEquip, which is
                // what bot.inventory.equipUsableItem does.
                bot.inventory.equipUsableItem(item.id);
            } else bot.inventory.equipItem(item.id);
            this.sleep(1500);
            if (bot.inventory.isEquipped(item.id)) this.logger(`Equipped item: ${item.name} (ID: ${item.id})`);

            this.sleep();
        }
    }

    /** Switches the player's alignment. */
    changeAlignment(side: Alignment): void {
        bot.send.packet(`%xt%zm%updateQuest%${bot.map.roomId}%41%${side}%`);
        this.sleep(this.actionDelay * 2);
    }

    /** Checks if a specific achievement is obtained. */
    hasAchievement(id: number, ia = "ia0"): boolean {
        return flag(bot.flash.callGameFunction("world.getAchievement", ia, id));
    }

    setAchievement(id: number, ia = "ia0"): void {
        if (!this.hasAchievement(id, ia)) bot.send.packet(`%xt%zm%setAchievement%${bot.map.roomId}%${ia}%${id}%1%`);
    }

    // C# WebClient / GetRequest: HTTP for SetOptions extras, dropped.

    /** C# `Bot.Reputation.GetRank(faction)` (API gap: read from `world.myAvatar.factions`). */
    factionRank(faction: string): number {
        const factions = bot.flash.get<Raw[] | null>("world.myAvatar.factions") ?? [];
        const found = factions.find((f) => f && text(f.sName) === faction);
        return found ? num(found.iRank) : 0;
    }

    // ---------------------------------------------------------------------------------------
    // Saved state (Farm helpers)
    // ---------------------------------------------------------------------------------------

    private savedStateEnabled = false;
    private previousMap = "";
    private nextSaveStateTime = 0;

    /**
     * Enables/disables automatic save-state jumps to /whitemap at random intervals (30 mins - 1 hour).
     * @param previousMap The map to return to after the save state.
     */
    savedState(on = true, previousMap = ""): void {
        // Enabling needs a map and is a no-op while already enabled.
        if (on ? !previousMap || this.savedStateEnabled : !this.savedStateEnabled) return;

        this.savedStateEnabled = on;
        this.logger(
            `SaveState [${this.savedStateEnabled ? "True" : "False"}] ` +
                (this.savedStateEnabled ? "We'll randomly goto /whitemap to save your progress every 30mns to an hour" : ""),
            "SaveStateHandler",
        );

        if (on) {
            this.previousMap = previousMap;
            this.nextSaveStateTime = Date.now() + this.getRandomInterval();
        } else this.savedStateEnabled = false;
    }

    /** A random interval between 30 minutes and 1 hour (ms). */
    private getRandomInterval(): number {
        return randomInt(30 * 60 * 1000, 60 * 60 * 1000 + 1);
    }

    /** Whether it's time to execute a save state during farming. */
    checkSaveState(): boolean {
        return this.savedStateEnabled && Date.now() >= this.nextSaveStateTime;
    }

    /** Executes the save state: goes to /whitemap and back to the previous map. */
    executeSaveState(): void {
        if (!this.previousMap || !this.savedStateEnabled) return;
        try {
            this.jump("Enter", "Spawn", false);
            this.join("whitemap-100000");
            if (this.previousMap) this.join(this.previousMap);
            this.nextSaveStateTime = Date.now() + this.getRandomInterval();
        } catch (error) {
            rethrowIfStopped(error);
            this.logger(`SaveState error: ${errorMessage(error)}`);
        }
    }

    // ---------------------------------------------------------------------------------------
    // AC banking
    // ---------------------------------------------------------------------------------------

    /** `bWear` and `EnhPatternID` of the inventory items (not in turu's InventoryItem). */
    private inventoryWearAndPattern(): Map<number, { wearing: boolean; enhancementPatternId: number }> {
        const result = new Map<number, { wearing: boolean; enhancementPatternId: number }>();
        for (const raw of bot.flash.get<Raw[] | null>("world.myAvatar.items") ?? [])
            if (raw) result.set(num(raw.ItemID), { wearing: flag(raw.bWear), enhancementPatternId: num(raw.EnhPatternID) });
        return result;
    }

    /**
     * Banks misc. AC-tagged items (notes, items, resources, quest items; server-use items too when
     * no boosts are configured), never equipped/blacklisted/configured-gear items.
     * @param requiredSpaces How many items to bank; 0 = all that match.
     */
    bankACMisc(requiredSpaces = 0, inventoryPool: string | null = null): void {
        const exemptIds = [18927, 38575]; // Treasure Potion, Dark Potion
        const allowedCategories = ["note", "item", "resource", "questitem"];

        // Include ServerUse if boosts are not active (Turu has no boost runner; CBO file absent).
        if (!["doGoldBoost", "doClassBoost", "doRepBoost", "doExpBoost"].some((key) => this.cboBool(key) === true))
            allowedCategories.push("serveruse");

        const extras = this.inventoryWearAndPattern();
        let toBankItems = bot.inventory.items.filter((item) => {
            const extra = extras.get(item.id);
            return (
                item.coins &&
                !item.equipped &&
                !(extra?.wearing ?? false) &&
                !item.meta &&
                (extra?.enhancementPatternId === 1 || item.enhancementLevel === 0) &&
                allowedCategories.some((c) => categoryIs(item, c)) &&
                !this.bankingBlackList.includes(item.name) &&
                !exemptIds.includes(item.id) &&
                !this.farmGear.includes(item.name) &&
                item.name !== this.farmClass &&
                !this.soloGear.includes(item.name) &&
                item.name !== this.soloClass &&
                !this.dodgeGear.includes(item.name) &&
                item.name !== this.dodgeClass &&
                !this.bossGear.includes(item.name) &&
                item.name !== this.bossClass
            );
        });

        // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
        toBankItems = this.filterInventoryPool(toBankItems, inventoryPool);
        if (toBankItems.length === 0) return;

        if (this.dlEnabled)
            for (const item of toBankItems)
                this.debugLogger(`DEBUG - Banking: ${item.name} | Equipped: ${item.equipped} | Wearing: ${extras.get(item.id)?.wearing} | ID: ${item.id}`);

        const selected = requiredSpaces > 0 ? toBankItems.slice(0, requiredSpaces) : toBankItems;
        this.logger(`Banking misc AC items [${selected.length} items]: ${selected.map((item) => `"${item.name}"`).join(", ")}`);
        this.toBank(...selected.map((item) => item.id));
    }

    /** Banks non-equipped AC house items. */
    bankACHouseItems(): void {
        const toHouseBank = bot.house.items.filter((item) => item.coins && !item.equipped).map((item) => item.id);
        if (toHouseBank.length > 0) {
            this.logger("Banking non-equipped house items", `${toHouseBank.length} items`);
            this.toHouseBank(...toHouseBank);
        }
    }

    /**
     * Banks unenhanced AC gear that isn't equipped or part of a configured gear set.
     * @param requiredSpaces Max number of items to bank; 0 means all.
     */
    bankACUnenhancedGear(requiredSpaces = 0, inventoryPool: string | null = null): void {
        const allProtectedGear = CoreBots.combineGearSets([this.soloGear, this.farmGear, this.bossGear, this.dodgeGear]);
        const extras = this.inventoryWearAndPattern();

        let bankableItems = bot.inventory.items.filter((item) => {
            const extra = extras.get(item.id);
            return (
                !categoryIs(item, "Class") &&
                item.coins &&
                // Bank lvl1 / non-level items & items with Adventure Enh
                (item.enhancementLevel <= 1 || extra?.enhancementPatternId === 1) &&
                !item.equipped &&
                !(extra?.wearing ?? false) &&
                !this.bankingBlackList.includes(item.name) &&
                !allProtectedGear.has(item.name.toLowerCase())
            );
        });

        // TEMPORARY Game4000 inventory change. Remove after the client inventory fix.
        bankableItems = this.filterInventoryPool(bankableItems, inventoryPool);

        if (bankableItems.length === 0) {
            this.logger("✅ Inventory clean — no bankable unenhanced AC gear found.");
            return;
        }

        const itemsToBank = requiredSpaces > 0 ? bankableItems.slice(0, requiredSpaces) : bankableItems;
        this.logger(`🏦 Banking unenhanced AC gear [${itemsToBank.length} items]: ${itemsToBank.map((i) => `"${i.name}" x${i.quantity}`).join(", ")}`);
        this.toBank(...itemsToBank.map((i) => i.id));
    }

    /** Lowercased names of all the gear sets (case-insensitive set). */
    private static combineGearSets(gearSets: string[][]): Set<string> {
        return new Set(gearSets.flat().filter((item) => item.trim().length > 0).map((item) => item.toLowerCase()));
    }

    static readonly reinstallCleanFlash =
        ". If the issue persists, try the following things in the order they are here:\n - Restart the client.\n - Restart your computer.\n - Reinstall the Turu client";

    // C# SkipOptions / RetryCellOnAFK: SetOptions extras, dropped.

    // ---------------------------------------------------------------------------------------
    // Maps
    // ---------------------------------------------------------------------------------------

    /**
     * Re-applies the level-100 bypass on level-locked maps (icestormunder, battlegrounde, ...)
     * when the server reset it, then jumps to `cellPad`.
     */
    byPassCheck(cellPad: [cell: string, pad: string]): void {
        if (!bot.player.alive || !bot.player.cell || !bot.player.pad) return;

        // Wait until the player is alive
        while (!bot.shouldExit && !bot.player.alive) this.sleep();

        // Attempt to get the level from Flash
        const bypassLevel = bot.flash.get<number | string | null>("world.myAvatar.objData.intLevel");
        if (bypassLevel === null || bypassLevel === undefined || bypassLevel === "") return;
        const flashLevel = Number.parseInt(text(bypassLevel), 10) || 0;

        if (flashLevel >= 100 || bot.player.level >= 100) return;

        const levelLockedMaps = [
            { map: "icestormunder", levelRequired: 75 },
            { map: "icewing", levelRequired: 75 },
            { map: "battlegrounde", levelRequired: 61 },
            { map: "voidxyfrag", levelRequired: 80 },
            { map: "voidnerfkitten", levelRequired: 80 },
        ];
        const mapName = bot.map.name;
        const currentMap = levelLockedMaps.find((m) => m.map === mapName);
        if (!currentMap || flashLevel >= currentMap.levelRequired || bot.player.level >= currentMap.levelRequired) return;

        if (flashLevel < currentMap.levelRequired) this.logger("Bypass Broke, resetting level");

        const previousMap = mapName;
        this.join("whitemap");
        this.join(previousMap);
        this.sleep();

        // Send a client packet to set the player's level to 100
        bot.send.clientPacket('{"t":"xt","b":{"r":-1,"o":{"cmd":"levelUp","intExpToLevel":"0","intLevel":100}}}', "json");
        this.sleep();

        this.jump(cellPad[0], cellPad[1]);
        this.sleep();
    }

    /** Jumps to the cell (case-corrected) and sets the spawn point. */
    jump(cell = "Enter", pad = "Spawn", ignoreCheck = false): void {
        if (equalsIgnoreCase(bot.player.cell, cell)) {
            bot.player.setSpawnPoint();
            return;
        }

        cell = bot.map.cells.find((c) => equalsIgnoreCase(c, cell)) ?? cell;
        pad = toTitleCase(pad);

        if (!ignoreCheck && bot.player.cell === cell) return;

        while (!bot.shouldExit && bot.player.cell !== cell) {
            if (cell && bot.player.cell !== cell) {
                bot.map.jump(cell, pad, this.inPublicRoom());
                bot.wait.forCellChange(cell || "Enter");
            }
            this.sleep();
            if (bot.player.cell === cell) break;
        }
        bot.player.setSpawnPoint();
    }

    /**
     * Jumps to a cell without monsters to get out of combat (or twice in the current cell when
     * there is none). Waits {@link exitCombatDelay} afterwards.
     */
    jumpWait(): void {
        if (bot.map.name === "battleon") return;

        // Disable all combat-related options
        bot.options.attackWithoutTarget = false;
        bot.options.aggroAllMonsters = false;
        bot.options.aggroMonsters = false;

        // C# builds a first blacklist here that ProceedToFilteringCases immediately rebuilds.
        this.proceedToFilteringCases();
    }

    private proceedToFilteringCases(): void {
        const cells = bot.map.cells;
        const playerCell = bot.player.cell;
        const blacklistPatterns = this.blackListedJumptoCells.map((pattern) => new RegExp(`^${pattern}$`, "i"));

        // Base blacklist: monster cells + regex rules + "Enter" cells when not in Enter
        const blackListed = new Set<string>(bot.monsters.mapMonsters.map((m) => m.cell.toLowerCase()));
        for (const cell of cells) {
            if (
                cell &&
                (/(^cut\w*$)|(^\w*cut$)|(^cut$)|(^r\d+$)|^(bs\d+|ar\d+|ms\d+|apo\d+|guild|move\d+|moveframe|game)$/i.test(cell) ||
                    blacklistPatterns.some((pattern) => pattern.test(cell)) ||
                    (playerCell !== "Enter" && cell.toLowerCase().includes("enter")))
            )
                blackListed.add(cell.toLowerCase());
        }
        const add = (...more: string[]) => more.forEach((c) => blackListed.add(c.toLowerCase()));

        switch (bot.map.name) {
            case "towerofmirrors":
                add("replay", "EndCuts");
                break;
            case "thevoid":
                add("r4");
                break;
            case "hydra":
                add("Boss");
                break;
            case "escherion":
                add(...cells.filter((c) => /^(Frame|e)\d+$/.test(c)), "Boss");
                break;
            case "originul":
                add("r10");
                break;
            case "yokaigrave":
                add("Quest");
                break;
            case "pyramid":
            case "mqlesson":
                add("Game");
                break;
            case "stalagbite":
                add("Enter", "r1", "CutShow");
                break;
            case "hbchallenge":
                add("r7");
                break;
            case "Gluttony":
                add(...cells.filter((x) => x.toLowerCase().startsWith("enter")));
                break;
            case "xantown":
                add(...cells.filter((x) => x && x !== "r12"));
                break;
            case "darkoviaforest":
            case "safiria":
            case "lycan":
                add("Quest");
                break;
            case "mobius":
                add("Hair");
                break;
            case "beehive":
                add("Dance");
                break;
            case "oaklore":
                add("Enter", "r1");
                break;
            case "pyrewatch":
                add("r3", "r4", "r5", "r7", "r12");
                break;
            case "shadowfall":
                add("New6");
                break;
            case "shadowattack":
                add("Enter", "After", "noChaos", "noDrak", "Cut38", "Cut39", "Cut1", "Boss");
                break;
            case "shadowrise":
                add("Enter", "After", "noDrak", "noChaos", "Cut1", "Cut37");
                break;
            case "bloodmoon":
                add("Enter", "r17");
                break;
            case "wanders":
                bot.map.jump("Boss", "Left", this.inPublicRoom());
                this.pause(2500);
                add(...(bot.player.cell === "Boss" ? ["r25", "Enter", "Enter2"] : ["Boss", "Enter", "Enter2"]));
                break;
            case "zephyrus":
                add("R1", "Enter");
                break;
            case "kitsune":
                add("Quest");
                break;
            case "portalundead":
                add("Portal", "Gate");
                break;
            case "icestormarena":
                add("r23");
                break;
            case "battlecon":
                add("rFight");
                break;
            case "necroU":
                add("Leave", "r6");
                break;
            case "finalbattle":
            case "confrontation":
                add("noChaos");
                break;
            case "gilead":
                add("Fail", "Cut1");
                break;
            case "mummies":
                add("moveFrame", "Cut1", "Cut2", "Cut3", "Cut4", "Cut5");
                break;
            default:
                break;
        }

        if (!bot.player.isMember) add("Eggs");

        // Viable cells: not in the static list nor the blacklist (case-insensitive)
        const staticList = new Set(this.blackListedJumptoCells.map((c) => c.toLowerCase()));
        const viableCells = unique(cells.filter((c) => !staticList.has(c.toLowerCase()) && !blackListed.has(c.toLowerCase())));

        const cellPad: [string, string] =
            viableCells.length > 0
                ? [viableCells[0], equalsIgnoreCase(bot.map.name, "battleon") ? "Spawn" : "Left"]
                : [bot.player.cell || "Enter", bot.player.pad || "Left"];

        this.performJump(cellPad, viableCells.length > 0 ? 1 : 2);
    }

    private lastMapJW = "";
    private lastCellPadJW: [string, string] = ["", ""];

    private performJump(cellPad: [string, string], jumpCount: number): void {
        const mapName = bot.map.name;
        if (this.lastMapJW !== mapName || this.lastCellPadJW[0] !== cellPad[0] || this.lastCellPadJW[1] !== cellPad[1]) {
            for (let i = 0; i < jumpCount; i++) {
                bot.map.jump(cellPad[0], cellPad[1], this.inPublicRoom());
                this.waitForTrue(() => bot.player.cell === cellPad[0], 20);
            }
            this.lastMapJW = mapName;
            this.lastCellPadJW = cellPad;
            this.sleep(this.exitCombatDelay < 200 ? this.exitCombatDelay : this.exitCombatDelay - 200);
        }
    }

    /** Cell names (and regex patterns) never jumped to. */
    blackListedJumptoCells: string[] = [
        "Wait",
        "Blank",
        "Out",
        "CutMikoOrochi",
        "innitRoom",
        "Video",
        "Leave",
        "moveFrame",
        "MoveFrame",
        "Moveframe",
        "Quest",
        "Game",
        "Fall",
        "Move",
        "Cut",
        "Movie",
        "movie",
        "^Frame\\d+$", // matches Frame1, Frame2, Frame10, etc.
        "^e\\d+$", // matches e1, e2, e55, etc.
    ];

    /**
     * Joins a map (in a private room when {@link privateRooms} is on) and does the extra steps
     * some maps need (quest-progress bypasses, prerequisites).
     *
     * Only the per-map cases the ported scripts reach are kept (stalagbite, djinn, towerofdoom*,
     * darkoviaforest/lycan/safiria, tercessuinotlim, oaklore, icestormarena and the "always
     * private" maps evilmarsh/nulgath); every other map goes through the generic join.
     * @param mapArg Map name ("map" or "map-1234").
     * @param cell The cell to jump to.
     * @param pad The pad to jump to.
     */
    join(mapArg: string | null | undefined, cell: string | null = "Enter", pad = "Spawn", options: JoinOptions = {}): void {
        const { publicRoom = false, ignoreCheck = false } = options;
        if (!mapArg) {
            this.logger("Map is null, cannot join.");
            return;
        }

        if (this.privateRooms && String(this.privateRoomNumber).length > 6)
            this.privateRoomNumber = Number.parseInt(String(this.privateRoomNumber).slice(0, 6), 10);

        let mapName = mapArg.replace(/ /g, "").replace(/I/g, "i");
        mapName = mapName.toLowerCase() === "tercess" ? "tercessuinotlim" : mapName.toLowerCase();
        let map: string = mapName;
        const strippedMap = map.includes("-") ? map.split("-")[0] : map;
        cell = bot.map.cells.find((c) => equalsIgnoreCase(c, cell)) ?? cell;
        pad = toTitleCase(pad);

        if (bot.map.name && bot.map.name.toLowerCase() === strippedMap && !ignoreCheck) return;

        // if aggro/aggroall is enabled when joining a map, disable it [forced]
        bot.options.aggroMonsters = false;
        bot.options.aggroAllMonsters = false;

        this.sleep();

        const tryJoin = (): void => {
            try {
                const offMemberLock = bot.events.on("extensionPacket", (packet) => {
                    try {
                        if (!packet) {
                            this.logger("Packet is null.");
                            return;
                        }
                        const type = packet?.params?.type;
                        const data = packet?.params?.dataObj;
                        if (type === "str" && Array.isArray(data) && data[0] === "warning" && JSON.stringify(packet).includes("is an Membership-Only Map")) {
                            offMemberLock();
                            this.logger(` "${map}" requires membership to access it. Stopping the Bot.`, "", { stopBot: true });
                        }
                    } catch (error) {
                        rethrowIfStopped(error);
                        this.logger(`An error occurred: ${errorMessage(error)}.`);
                    }
                });

                try {
                    // Detect if map string already contains a numeric suffix
                    const suffix = Number.parseInt(map.split("-").pop() ?? "", 10);
                    const hasMapNumber = map.includes("-") && Number.isFinite(suffix) && suffix >= 1000;

                    for (let i = 0; i < 20; i++) {
                        bot.wait.forActionCooldown("tfer");

                        // Join if we are not in the desired map
                        if (map && bot.map.name !== map) {
                            const target = hasMapNumber || (publicRoom && this.publicDifficult) || !this.privateRooms ? map : `${map}-${this.privateRoomNumber}`;
                            bot.map.join(target, cell ?? "Enter", cell === null ? "Spawn" : pad ?? "Left", false, this.inPublicRoom());

                            // Wait for the correct map (full name, not stripped)
                            bot.wait.forMapLoad(map);

                            // Exponential backoff on retries
                            this.sleep(Math.max(1, 100 * randomInt(0, Math.trunc(Math.pow(2, i / 2)))));
                        }

                        // Update blacklisted cells
                        this.blackListedJumptoCells = unique([
                            ...this.blackListedJumptoCells,
                            ...bot.map.cells.filter((x) => x && /(^cut\w*$)|(^\w*cut$)|(^cut$)|^r\d+$|^(bs\d+|ar\d+|ms\d+|apo\d+|guild)$/i.test(x)),
                        ]);

                        // Auto-cell selection for maps (skip oaklore)
                        if (map !== "oaklore" && (cell === null || cell === "Enter")) {
                            const cells = bot.map.cells;
                            const validCells = cells.filter(
                                (x) => x && !this.blackListedJumptoCells.some((b) => x.toLowerCase().includes(b.toLowerCase())),
                            );
                            cell = cells.filter((x) => x.toLowerCase().includes("enter")).length > 1 ? validCells[0] ?? null : "Enter";

                            if (cell !== null && bot.player.cell !== cell) {
                                bot.map.jump(cell, pad ?? "Spawn", false);
                                this.sleep(1000);
                            }
                        }

                        // Break out if we landed in the stripped map name
                        if (equalsIgnoreCase(bot.map.name, strippedMap)) {
                            if (bot.options.safeTimings) {
                                if (map && !bot.wait.forMapLoad(map, 20)) {
                                    if (cell !== null && bot.player.cell !== cell) bot.map.jump(bot.player.cell, bot.player.pad, this.inPublicRoom());
                                } else if (cell !== null && bot.player.cell !== cell) {
                                    bot.map.jump(cell, pad ?? "Spawn", this.inPublicRoom());
                                }
                                this.sleep();
                                bot.wait.forCellChange(cell ?? "Enter");
                            }
                            break;
                        }

                        if (i === 19) this.logger(`Failed to join ${map}`);
                    }
                } finally {
                    offMemberLock();
                }
            } catch (error) {
                rethrowIfStopped(error);
                this.logger(`An error occurred: ${errorMessage(error)}.`);
            }
        };

        /** Sets quest-chain values (C# `Bot.Quests.UpdateQuest(value, slot)`) then joins. */
        const simpleQuestBypass = (...slotValues: [slot: number, value: number][]): void => {
            try {
                for (const [slot, value] of slotValues)
                    bot.send.clientPacket(`{"t":"xt","b":{"r":-1,"o":{"cmd":"updateQuest","iValue":${value},"iIndex":${slot}}}}`, "json");
                this.sleep();
                tryJoin();
            } catch (error) {
                rethrowIfStopped(error);
                this.logger(`An error occurred: ${errorMessage(error)}`);
            }
        };

        switch (strippedMap) {
            // Simple Quest Bypasses (C# has many more maps; only those the ported scripts visit are kept)
            case "stalagbite":
                simpleQuestBypass([22, 35]);
                break;

            case "pyramid":
            case "djinn":
                simpleQuestBypass([36, 28]);
                break;

            case "towerofdoom":
            case "towerofdoom2":
            case "towerofdoom3":
            case "towerofdoom4":
            case "towerofdoom5":
            case "towerofdoom6":
            case "towerofdoom7":
            case "towerofdoom8":
            case "towerofdoom9":
            case "towerofdoom10":
                simpleQuestBypass([159, 10]);
                break;

            case "darkoviaforest":
            case "lycan":
            case "safiria":
                simpleQuestBypass([26, 23]);
                break;

            // Special Cases
            case "tercessuinotlim": {
                const prereqQuest1 = bot.quests.hasBeenCompleted(9540);
                const prereqQuest2 = bot.quests.hasBeenCompleted(9541);

                if (!prereqQuest1) {
                    this.oneTimeMessage(
                        "WARNING!",
                        'This map now requires a one-time completion of "Beyond the Portal"\nNot sure why it loads tercessuinotlim first, but get over it :|',
                        false,
                    );
                    for (const [slot, value] of [
                        [15, 8],
                        [542, 2],
                    ] as const)
                        bot.send.clientPacket(`{"t":"xt","b":{"r":-1,"o":{"cmd":"updateQuest","iValue":${value},"iIndex":${slot}}}}`, "json");
                    this.sleep();
                    tryJoin();
                    this.join("citadel");
                    this.jump("m22", "Left");
                    this.equipClass(ClassType.Solo);
                    this.ensureAccept(9540);
                    this.killMonster("citadel", "m22", "Left", "Death's Head", "Death's Head Bested");
                    this.ensureComplete(9540);
                }

                if (!prereqQuest2) this.chainComplete(9541);

                this.jump("m22", "Left");
                tryJoin();
                break;
            }

            // Quest Prog swaps spawn cell
            case "oaklore":
                if (!cell || cell === "Enter") cell = "r1";
                tryJoin();
                break;

            case "icestormarena":
                this.jumpWait();
                if (bot.map.name && bot.map.name !== map)
                    bot.map.join(this.privateRooms ? `${map}-${this.privateRoomNumber}` : map, "Enter", "Spawn", false, this.inPublicRoom());
                bot.wait.forMapLoad("icestormarena");
                bot.send.clientPacket('{"t":"xt","b":{"r":-1,"o":{"cmd":"levelUp","intExpToLevel":"0","intLevel":100}}}', "json");
                this.sleep();
                if (cell !== null && bot.player.cell !== cell) bot.map.jump(cell ?? "Enter", pad, this.inPublicRoom());
                bot.wait.forCellChange(cell ?? "Enter");
                break;

            // Always Private (room limit 2; C# lists more maps)
            case "evilmarsh":
            case "nulgath":
                this.jumpWait();
                map = strippedMap + "-" + "100000";
                tryJoin();
                break;

            default:
                if (bot.map.name === "pyrewatch") this.jumpWait();
                tryJoin();
                break;
        }

        if (strippedMap === bot.map.name?.toLowerCase()) {
            // C# ButlerOnMe() / Butler lock file: dropped.
            if (cell !== null && bot.player.cell !== cell) bot.map.jump(cell, pad, this.inPublicRoom());
            this.sleep(1500);
        }
    }

    /** Joins a map, then loads another SWF into it (`world.loadMap`) and jumps to the cell. */
    joinSWF(map: string, swfPath: string, cell = "Enter", pad = "Spawn", ignoreCheck = false): void {
        this.join(map, cell, pad, { ignoreCheck, publicRoom: false });
        bot.wait.forMapLoad(map);
        bot.flash.callGameFunction("world.loadMap", swfPath);
        this.sleep(1500);
        // Wait until the player is fully loaded or exit condition is met
        while (!bot.shouldExit && !bot.player.loaded) this.sleep();

        const targetPad = cell === "Enter" ? "Spawn" : "Left";
        if (bot.player.cell !== cell) bot.map.jump(cell, targetPad, this.inPublicRoom());
        bot.wait.forCellChange(cell);
        // C# retries when Bot.Map is null, which never happens here.
    }

    // C# GetMapItem: only used by Join map cases this port leaves out, dropped.

    /** Whether the room you're in is a public room (room number below 1000). */
    inPublicRoom(): boolean {
        bot.wait.forMapLoad(bot.map.name);
        const nr = Number.parseInt(bot.map.fullName.split("-").pop() ?? "", 10);
        return (Number.isFinite(nr) ? nr : 1) < 1000;
    }

    /**
     * Resets a quest: abandons it (accepting it first if needed) and accepts it again.
     * Dark Makai and their Sigils/Runes need this (Swindles Return: 7551, Diamond Exchange: 869).
     */
    resetQuest(questId = 0): boolean {
        this.initializeWithRetries(() => bot.quests.ensureLoad(questId));

        if (bot.quests.active.some((q) => q.id === questId)) this.abandonQuest(questId);
        else {
            this.ensureAccept(questId);
            this.abandonQuest(questId);
        }
        this.ensureAccept(questId);
        return bot.quests.ensureAccept(questId);
    }

    /** Kills Dark Makai (tercessuinotlim, or nulgath/evilmarsh) until you have the item. */
    darkMakaiItem(item: string | null = null, quantity = 1, isTemp = true): void {
        if (!item || (isTemp ? bot.tempInv.contains(item, quantity) : this.checkInventory(item, quantity))) return;

        const maps: [string, string][] = [
            ["tercessuinotlim", "m1"],
            [this.isMember ? "Nulgath" : "evilmarsh", "Field1"],
        ];
        const selectedMap = maps[randomInt(0, maps.length)];

        this.join(selectedMap[0], selectedMap[1], "Left");

        // C# precedence kept: (!ShouldExit && isTemp) ? !TempInv.Contains : !Inventory.Contains
        while (!bot.shouldExit && isTemp ? !bot.tempInv.contains(item, quantity) : !bot.inventory.contains(item, quantity)) {
            if (bot.player.cell !== selectedMap[1]) this.jump(selectedMap[1]);
            bot.combat.attack("Dark Makai");
            this.sleep();
        }
        this.equipClass(ClassType.Solo);
    }

    // C# AutoReport / LockedQuestData, Butler log helpers, WriteFile, CollectData: crash
    // reports, Butler and telemetry, dropped.

    // ---------------------------------------------------------------------------------------
    // CoreBots Options values (C# CBO*) and one-time messages
    // ---------------------------------------------------------------------------------------

    /**
     * C# `ReadCBO`: reads your CoreBots Options file (Options > CoreBots) and applies it to the
     * `[Can Change]` fields. Does nothing when there is no file. The old dodge/boss gear keys are
     * renamed in the file first (C# `MigrateCBOKeys`, done by the engine).
     */
    readCbo(): void {
        const lines = bot.coreBotsOptions();
        if (lines === null) return;
        this.cboLines = lines;

        const bool = (name: string, set: (value: boolean) => void) => {
            const value = this.cboBool(name);
            if (value !== null) set(value);
        };
        const int = (name: string, set: (value: number) => void) => {
            const value = this.cboInt(name);
            if (value !== null) set(value);
        };
        const str = (name: string, set: (value: string) => void) => {
            const value = this.cboString(name);
            if (value !== null) set(value);
        };
        // C# Enum.Parse(typeof(ClassUseMode), ...); an unknown mode is Base here instead of a crash.
        const mode = (value: string): ClassUseMode =>
            (Object.values(ClassUseMode) as string[]).includes(value) ? (value as ClassUseMode) : ClassUseMode.Base;

        //Generic
        bool("PrivateRooms", (v) => (this.privateRooms = v));
        int("PrivateRoomNr", (v) => (this.privateRoomNumber = v));
        bool("PublicDifficult", (v) => (this.publicDifficult = v));
        bool("BankMiscAC", (v) => (this.bankMiscAC = v));
        bool("BankUnenhancedACGear", (v) => (this.bankUnenhancedACGear = v));
        bool("LoggerInChat", (v) => (this.loggerInChat = v));

        str("StopLocationSelect", (v) => (this.customStopLocation = v));

        str("SoloClassSelect", (v) => (this.soloClass = v));
        bool("SoloEquipCheck", (v) => (this.soloGearOn = v));
        str("SoloModeSelect", (v) => (this.soloUseMode = mode(v)));

        str("FarmClassSelect", (v) => (this.farmClass = v));
        bool("FarmEquipCheck", (v) => (this.farmGearOn = v));
        str("FarmModeSelect", (v) => (this.farmUseMode = mode(v)));

        str("DodgeClassSelect", (v) => (this.dodgeClass = v));
        bool("DodgeEquipCheck", (v) => (this.dodgeGearOn = v));
        str("DodgeModeSelect", (v) => (this.dodgeUseMode = mode(v)));

        str("BossClassSelect", (v) => (this.bossClass = v));
        bool("BossEquipCheck", (v) => (this.bossGearOn = v));
        str("BossModeSelect", (v) => (this.bossUseMode = mode(v)));

        //Advanced (C# assigns the "Disable..." values as they are)
        bool("DisableBestGear", (v) => (this.bestGear = v));
        bool("DisableAutoEnhance", (v) => (this.autoEnhance = v));
        bool("MessageBoxCheck", (v) => (this.forceOffMessageboxes = v));
        bool("RestCheck", (v) => (this.shouldRest = v));
        bool("AntiLag", (v) => (this.antiLag = v));

        int("ActionDelay", (v) => (this.actionDelay = v));
        int("ExitCombatNr", (v) => (this.exitCombatDelay = v));
        int("HuntDelayNr", (v) => (this.huntDelay = v));
        int("QuestTriesNr", (v) => (this.acceptandCompleteTries = v));
        int("QuestMaxNr", (v) => (this.loadedQuestLimit = v));

        //Class Equipment
        const gear = (on: boolean, n: number): string[] =>
            on
                ? ["Helm", "Armor", "Cape", "Weapon", "Pet", "GroundItem"]
                      .map((slot) => this.cboString(`${slot}${n}Select`))
                      .filter((name): name is string => name !== null)
                : [];
        const soloGear = gear(this.soloGearOn, 1);
        if (soloGear.length > 0) this.soloGear = soloGear;
        const farmGear = gear(this.farmGearOn, 2);
        if (farmGear.length > 0) this.farmGear = farmGear;
        const dodgeGear = gear(this.dodgeGearOn, 3);
        if (dodgeGear.length > 0) this.dodgeGear = dodgeGear;
        const bossGear = gear(this.bossGearOn, 4);
        if (bossGear.length > 0) this.bossGear = bossGear;
    }

    /** C# `CBOString`: the value of a CoreBots Options key, or null when not set. */
    cboString(name: string): string | null {
        const key = Object.keys(this.cboValues).find((k) => k.startsWith(name));
        let value = key === undefined ? undefined : this.cboValues[key];
        if (value === undefined) {
            const line = this.cboLines.find((l) => l.startsWith(name));
            value = line === undefined ? undefined : line.split(": ")[1];
        }
        return value === undefined || value === "fail" || value.trim().length === 0 ? null : value;
    }

    /** C# `CBOInt`: the number of a CoreBots Options key, or null when not set or not a number. */
    cboInt(name: string): number | null {
        const value = this.cboString(name);
        return value !== null && /^\s*[+-]?\d+\s*$/.test(value) ? parseInt(value, 10) : null;
    }

    /** C# `CBOBool`: true/false when set ("True" is true), null when not set. */
    cboBool(name: string): boolean | null {
        const value = this.cboString(name);
        return value === null ? null : value === "True";
    }

    private readonly oneTimeMessages = new Set<string>();

    /**
     * Logs a message once per script run (C# keeps a OneTimeMessages.txt file; Turu keeps it in
     * memory). Message boxes don't exist in Turu, so it never returns "Yes".
     */
    oneTimeMessage(internalName: string, message: string, _messageBox = true, _forcedMessageBox = false, _yesAndNo = false): boolean {
        if (this.oneTimeMessages.has(internalName)) return false;
        this.logger("Please make sure you read this as it will only be shown once:\n\n" + message, "One Time-Only Message");
        this.oneTimeMessages.add(internalName);
        return false;
    }

    // C# AprilFools / EnforceInvariantCulture / UserSpecificMessages: dropped.
}

/** The shared CoreBots instance (C# `CoreBots.Instance`). */
export const Core = CoreBots.instance;
