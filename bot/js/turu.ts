/**
 * Turu's script API — `import { bot } from "turu";`
 *
 * Every call is synchronous: it blocks until the game answers, so scripts read top to
 * bottom with no `await`. Pressing Stop makes the next call throw `ScriptStopped`.
 *
 * This file is the runtime (embedded in the app) and the type definitions (scripts/types
 * re-exports it for editors). Each method calls the Rust engine through
 * `__turu("module.method", args)`; the Rust side is bot/src/api/<module>.rs.
 * Names follow VibeSkua's `IScriptInterface`, in camelCase.
 */

declare function __turu(method: string, argsJson: string): string;
declare function __turuEvents(): string;

// ---------------------------------------------------------------------------------------
// Errors and the native bridge
// ---------------------------------------------------------------------------------------

/** Thrown by every bot call once the script is stopped. Don't catch it (or rethrow it). */
export class ScriptStopped extends Error {
    constructor() {
        super("ScriptStopped");
        this.name = "ScriptStopped";
    }
}

/** A bot call failed (bad argument, game not running, ...). */
export class BotError extends Error {
    constructor(message: string) {
        super(message);
        this.name = "BotError";
    }
}

function call<T>(method: string, ...args: unknown[]): T {
    const raw = __turu(method, JSON.stringify(args));
    const result = JSON.parse(raw) as { ok?: T; error?: string; stopped?: boolean; due?: string[] };
    if (result.stopped) throw new ScriptStopped();
    if (result.error !== undefined) throw new BotError(`${method}: ${result.error}`);
    dispatchEvents();
    if (result.due && result.due.length > 0) runDueHandlers(result.due);
    return result.ok as T;
}

// ---------------------------------------------------------------------------------------
// Game data
// ---------------------------------------------------------------------------------------

/** An item in the inventory, bank, temp inventory, house, a drop or a shop. */
export interface InventoryItem {
    id: number;
    charItemId: number;
    name: string;
    description: string;
    quantity: number;
    maxStack: number;
    level: number;
    enhancementLevel: number;
    /**
     * The enhancement type (`EnhPatternID`): 2 Fighter, 3 Thief, 5 Hybrid, 6 Wizard, 7 Healer,
     * 8 SpellBreaker, 9 Lucky; or the cape/helm special: 10 Forge, 11 Absolution, 12 Avarice,
     * 24 Vainglory, 25 Vim, 26 Examen, 27 Pneuma, 28 Anima, 29 Penitence, 30 Lament, 32 Hearty.
     */
    enhancementPatternId: number;
    /**
     * The weapon special (`ProcID`): 2 Spiral Carve, 3 Awe Blast, 4 Health Vamp, 5 Mana Vamp,
     * 6 Powerword Die, 7 Lacerate, 8 Smite, 9 Valiance, 10 Arcana's Concerto, 11 Acheron,
     * 12 Elysium, 13 Praxis, 14 Dauntless, 15 Ravenous.
     */
    procId: number;
    equipped: boolean;
    /** Member-only item. */
    upgrade: boolean;
    /** Costs AC. */
    coins: boolean;
    temp: boolean;
    house: boolean;
    /** "Class", "Sword", "Item", "Resource", "Quest Item", ... */
    category: string;
    /** `sES` (VibeSkua `ItemGroup`): "co" armor/class, "ba" cape, "he" helm, "pe" pet, "Weapon". */
    equipSlot: string;
    meta: string;
}

export interface Monster {
    /** Monster type ID (`MonID`). */
    id: number;
    /** ID of this monster in the map (`MonMapID`). */
    mapId: number;
    name: string;
    race: string;
    cell: string;
    hp: number;
    maxHp: number;
    /** 0 dead, 1 idle, 2 in combat. */
    state: number;
    level: number;
}

/** A badge on your character page (CoreBots `Badge`). */
export interface Badge {
    id: number;
    name: string;
    category: string;
}

/** A faction you have reputation in (VibeSkua `Faction`). */
export interface Faction {
    id: number;
    name: string;
    rank: number;
    /** The total amount of rep you have for this faction. */
    totalRep: number;
    /** The amount of rep you have for your current rank. */
    rep: number;
    /** The rep needed to rank up from your current rank. */
    requiredRep: number;
    /** `requiredRep - rep`. */
    remainingRep: number;
}

/** An aura on you or a monster (VibeSkua `Aura`). */
export interface Aura {
    name: string;
    /** The aura's value; its stack count for most auras. */
    value: number;
    type: string;
    category: string;
    /** Seconds it lasts. */
    duration: number;
    /** When it was applied, in Unix milliseconds (0 when unknown). */
    timeStamp: number;
    passive: boolean;
    potionType: string;
    icon: string;
}

/** An aura as the HUD shows it (VibeSkua `AuraSnapshot`). */
export interface AuraSnapshot {
    name: string;
    stacks: number;
    /** Seconds it lasts. */
    duration: number;
    /** Seconds left. */
    remainingTime: number;
    persistent: boolean;
    icon: string;
    description: string;
}

export interface QuestItem {
    id: number;
    name: string;
    quantity: number;
    temp: boolean;
}

export interface Quest {
    id: number;
    name: string;
    /** "c" when it can be completed. */
    status: string | null;
    /** Can only be completed once. */
    once: boolean;
    /** Member-only. */
    upgrade: boolean;
    level: number;
    /** Items needed to turn it in. */
    requirements: QuestItem[];
    /** Items needed to accept it. */
    acceptRequirements: QuestItem[];
    rewards: QuestItem[];
    /** Rewards you pick one of (`oRewards`), by item ID. */
    simpleRewards: QuestItem[];
    slot: number;
    value: number;
    field: string;
    index: number;
}

export interface ShopItem extends InventoryItem {
    shopItemId: number;
    cost: number;
    /** Quest/faction requirement text, if any. */
    requirements: string;
}

/** A game server (VibeSkua `Server`). */
export interface Server {
    name: string;
    ip: string;
    port: number;
    online: boolean;
    /** Member-only. */
    upgrade: boolean;
    playerCount: number;
    maxPlayers: number;
    /** "en" or "pt". */
    language: string;
    /** Canned chat 0, free chat 2. */
    chatLevel: number;
    level: number;
}

export interface Skill {
    index: number;
    name: string;
    manaCost: number;
    cooldown: number;
    isOk: boolean;
}

// ---------------------------------------------------------------------------------------
// Sub-APIs
// ---------------------------------------------------------------------------------------

/** `bot.player`: you. */
export class Player {
    get loggedIn(): boolean { return call("player.loggedIn"); }
    /** Logged in and alive. */
    get playing(): boolean { return call("player.playing"); }
    get alive(): boolean { return call("player.alive"); }
    get inCombat(): boolean { return call("player.inCombat"); }
    /** 0 dead, 1 idle, 2 in combat. */
    get state(): number { return call("player.state"); }
    /** Inventory loaded, no map loading, avatar art loaded. */
    get loaded(): boolean { return call("player.loaded"); }
    get afk(): boolean { return call("player.afk"); }
    get id(): number { return call("player.id"); }
    get username(): string { return call("player.username"); }
    get serverIp(): string { return call("player.serverIp"); }
    /** The login screen shows the "you were kicked" warning. */
    get kicked(): boolean { return call("player.kicked"); }
    get cell(): string { return call("player.cell"); }
    get pad(): string { return call("player.pad"); }
    get level(): number { return call("player.level"); }
    get health(): number { return call("player.health"); }
    get maxHealth(): number { return call("player.maxHealth"); }
    get mana(): number { return call("player.mana"); }
    get maxMana(): number { return call("player.maxMana"); }
    get gold(): number { return call("player.gold"); }
    get isMember(): boolean { return call("player.isMember"); }
    /**
     * Your character's badges from its character page on account.aq.com (CoreBots `Badges`).
     * Each read is a web request; empty before you're logged in.
     */
    get webBadges(): Badge[] { return call("player.webBadges"); }
    /** Name of the equipped class, or null. */
    get currentClass(): string | null { return call("player.currentClass"); }
    get currentClassRank(): number { return call("player.currentClassRank"); }
    /** The monster you target, or null. */
    get target(): Monster | null { return call("player.target"); }
    get hasTarget(): boolean { return call("player.hasTarget"); }
    get skills(): Skill[] { return call("player.skills"); }
    get x(): number { return call("player.x"); }
    get y(): number { return call("player.y"); }
    get walkSpeed(): number { return call("player.walkSpeed"); }
    set walkSpeed(value: number) { call("player.setWalkSpeed", value); }

    /** Rests; with `full`, waits until HP and MP are full. */
    rest(full = false): void { call("player.rest", full); }
    setSpawnPoint(cell?: string, pad?: string): void { call("player.setSpawnPoint", cell ?? null, pad ?? null); }
    walkTo(x: number, y: number, speed = 8): void { call("player.walkTo", x, y, speed); }
    /** Teleports to another player in the same server (`world.goto`). */
    goto(name: string): void { call("player.goto", name); }
}

/**
 * `bot.servers`: logging in and out, and relogins (VibeSkua `Bot.Servers`). The relogins log
 * out, log back in with the game's remembered login (or `setLoginInfo`'s) and connect;
 * auto-relogin (`bot.options.autoRelogin`) is off while they run. A server is a `Server` or a
 * name.
 */
export class Servers {
    /** The servers the game's login screen lists (`serialCmd.servers`). */
    get serverList(): Server[] { return call("servers.serverList"); }
    /** The servers from the last `getServers` (the game's server list API). */
    get cachedServers(): Server[] { return call("servers.cachedServers"); }
    /** Fetches the server list from the game's API (the cached one unless `forceUpdate`). */
    getServers(forceUpdate = false): Server[] { return call("servers.getServers", forceUpdate); }
    /** The IP of the server you were last on. */
    get lastIp(): string { return call("servers.lastIp"); }
    /** The name of the server you were last on. */
    get lastName(): string { return call("servers.lastName"); }
    /** You logged out yourself (auto-relogin doesn't bring you back). */
    get isIntentionalLogout(): boolean { return call("servers.isIntentionalLogout"); }
    set isIntentionalLogout(value: boolean) { call("servers.setIntentionalLogout", value); }
    /** Connected to a game server. */
    get isConnected(): boolean { return call("servers.isConnected"); }
    /** The `setLoginInfo` username, else the game's; null before a login. */
    get cachedUsername(): string | null { return call("servers.cachedUsername"); }

    /** The login relogins use instead of the game's remembered one. */
    setLoginInfo(username: string, password: string): void { call("servers.setLoginInfo", username, password); }
    /** Logs in (to the server list); without arguments, with the game's remembered login. */
    login(username?: string, password?: string): void { call("servers.login", username ?? null, password ?? null); }
    /** Logs out to the login screen. Auto-relogin doesn't bring you back. */
    logout(): void { call("servers.logout"); }
    /** Back to the login screen without an intentional logout. */
    disconnectForRelogin(): void { call("servers.disconnectForRelogin"); }
    /** From the server list: connects to a server and waits for the world (`loginTimeout`). */
    connect(server: string | Server): boolean { return call("servers.connect", server); }
    connectIp(ip: string, port?: number): boolean { return call("servers.connectIp", ip, port ?? null); }
    /** Logs in, waits `loginDelay` ms and connects. */
    reconnect(server: string | Server, loginDelay = 2000): boolean { return call("servers.reconnect", server, loginDelay); }
    /**
     * Logs out and back in, on `server` when it can be used (online, not full, not a test
     * server, not member-only for free players), else on the best other one. Returns true
     * when you're back in game.
     */
    relogin(server?: string | Server): boolean { return call("servers.relogin", server ?? null); }
    reloginIp(ip: string): boolean { return call("servers.reloginIp", ip); }
    /**
     * Logs in, trying other servers, up to `options.reloginTries` times. Does nothing while
     * you're playing: log out (or `disconnectForRelogin`) first.
     */
    ensureRelogin(serverName?: string): boolean { return call("servers.ensureRelogin", serverName ?? null); }
}

/** `bot.map`: the current map. */
export class GameMap {
    /** Map name without the room number, e.g. "battleon". */
    get name(): string { return call("map.name"); }
    get fullName(): string { return call("map.fullName"); }
    get roomId(): number { return call("map.roomId"); }
    get loading(): boolean { return call("map.loading"); }
    get loaded(): boolean { return call("map.loaded"); }
    get cells(): string[] { return call("map.cells"); }
    get playerNames(): string[] { return call("map.playerNames"); }
    get playerCount(): number { return call("map.playerCount"); }
    get filePath(): string | null { return call("map.filePath"); }
    get lastMap(): string | null { return call("map.lastMap"); }

    /**
     * Joins a map ("battleon", or "battleon-1234" for a room) and jumps to cell/pad.
     * Uses a private room when `bot.options.privateRooms` is on.
     */
    join(map: string, cell = "Enter", pad = "Spawn", ignoreCheck = false, autoCorrect = true): void {
        call("map.join", map, cell, pad, ignoreCheck, autoCorrect);
    }
    /** Moves to a cell of the current map. */
    jump(cell: string, pad = "Left", autoCorrect = true, clientOnly = false): void {
        call("map.jump", cell, pad, autoCorrect, clientOnly);
    }
    reload(): void { call("map.reload"); }
    /** Picks up a map item (`world.getMapItem`). */
    getMapItem(id: number): void { call("map.getMapItem", id); }
}

/** `bot.reputation` (VibeSkua `Bot.Reputation`): your factions and ranks. */
export class Reputation {
    /** Every faction you have at least 1 point of reputation in. */
    get factionList(): Faction[] { return call("reputation.factionList"); }
    /** Your rank in a faction, by its exact name or its ID; 0 without reputation in it. */
    getRank(faction: string | number): number { return call("reputation.getRank", faction); }
    /** Whether your rank in the faction is at least `rank`. */
    hasRank(faction: string | number, rank = 1): boolean { return call("reputation.hasRank", faction, rank); }
}

/** The aura members `bot.self` and `bot.target` share (VibeSkua `IScriptSelfAuras`/`IScriptTargetAuras`). */
abstract class SubjectAuras {
    protected abstract readonly module: "self" | "target";
    /** The active auras (names compare without case below). */
    get auras(): Aura[] { return call(`${this.module}.auras`); }
    /**
     * The auras as the HUD shows them, with their stacks and time left. A timed aura whose time
     * ran out is left out even if the HUD still has it (VibeSkua keeps it), so skill rules and
     * {@link getAuraStacks} don't count stale stacks.
     */
    get snapshots(): AuraSnapshot[] { return call(`${this.module}.snapshots`); }
    /** An active aura by name, or null. */
    getAura(name: string): Aura | null { return call(`${this.module}.getAura`, name); }
    /** An aura of {@link snapshots} by name, or null. */
    getAuraSnapshot(name: string): AuraSnapshot | null { return call(`${this.module}.getAuraSnapshot`, name); }
    /** The stacks of an aura (0 when it isn't there). */
    getAuraStacks(name: string): number { return call(`${this.module}.getAuraStacks`, name); }
    /** Whether the aura is active. */
    hasActiveAura(name: string): boolean { return call(`${this.module}.hasActiveAura`, name); }
    /** The aura's value as skua.swf sums it (`GetAurasValue`); 0 when it isn't there. */
    getAuraValue(name: string): number { return call(`${this.module}.getAuraValue`, name); }
    /** Whether any of these auras is active. */
    hasAnyActiveAura(...names: string[]): boolean { return call(`${this.module}.hasAnyActiveAura`, names); }
}

/** `bot.self` (VibeSkua `Bot.Self`): the auras on you. */
export class SelfAuras extends SubjectAuras {
    protected readonly module = "self";
    /** The auras on a player in your map, by name (C# returns the raw JSON). */
    getPlayerAura(playerName: string): Aura[] { return call("self.getPlayerAura", playerName); }
}

/** `bot.target` (VibeSkua `Bot.Target`): the auras on your target. */
export class TargetAuras extends SubjectAuras {
    protected readonly module = "target";
    /** The auras on a monster, by name or map ID (C# returns the raw JSON). */
    getMonsterAura(monster: string | number): Aura[] { return call("target.getMonsterAura", monster); }
}

/** `bot.combat`: targeting and attacking. */
export class Combat {
    /** Targets and attacks a monster in your cell, by name ("*" for any) or map ID. */
    attack(monster: string | number): boolean { return call("combat.attack", monster); }
    attackPlayer(name: string): void { call("combat.attackPlayer", name); }
    approachTarget(): void { call("combat.approachTarget"); }
    cancelTarget(): void { call("combat.cancelTarget"); }
    cancelAutoAttack(): void { call("combat.cancelAutoAttack"); }
    untargetSelf(): void { call("combat.untargetSelf"); }
    /** Leaves combat (jumps in place until the player is idle). */
    exit(): void { call("combat.exit"); }
    /**
     * While true, {@link attack} cancels the target and returns false, and the skill timer
     * holds its skills until a counter attack fades (10 s at most, then it clears this). A
     * death or the end of the script clears it.
     */
    get stopAttacking(): boolean { return call("combat.stopAttacking"); }
    set stopAttacking(value: boolean) { call("combat.setStopAttacking", value); }
    /**
     * When a monster prepares a counter attack, sets {@link stopAttacking} and drops the target;
     * once its "Counter Attack" aura fades, attacks that target again. Usually turned on with
     * `bot.ultraBossHelper.enableCounterAttack()`.
     */
    get enableCounterHandler(): boolean { return call("combat.enableCounterHandler"); }
    set enableCounterHandler(value: boolean) { call("combat.setEnableCounterHandler", value); }
}

/** `bot.ultraBossHelper` (VibeSkua `Bot.UltraBossHelper`): the counter-attack pause and aura checks for ultra bosses. */
export class UltraBossHelper {
    /**
     * Turns the counter-attack pause on ({@link Combat.enableCounterHandler}): attacks stop while
     * a monster's counter attack is up and resume on the same target after. It stays on after
     * the script ends, as in VibeSkua; CoreBots' `setOptions()` turns it on.
     */
    enableCounterAttack(): void { call("combat.enableCounterAttack"); }
    /** Turns the counter-attack pause off. */
    disableCounterAttack(): void { call("combat.disableCounterAttack"); }
    /** Whether {@link enableCounterAttack} is on. */
    get isCounterAttackEnabled(): boolean { return call("combat.isCounterAttackEnabled"); }
    /** VibeSkua's flag for a counter attack in progress (VibeSkua never sets it, so it reads false). */
    get isCounterAttackActive(): boolean { return call("combat.isCounterAttackActive"); }
    /** Sets {@link Combat.stopAttacking} by hand, for your own counter-attack logic. */
    setAttacksStopped(shouldStop: boolean): void { call("combat.setAttacksStopped", shouldStop); }
    /**
     * Which of a positive and a negative charge aura are active, and whether either one with
     * `reversedSuffix` appended (e.g. "?") is.
     */
    analyzeChargeMechanics(
        auras: SelfAuras | TargetAuras, positiveChargeName: string, negativeChargeName: string, reversedSuffix?: string,
    ): { hasPositive: boolean; hasNegative: boolean; hasReversed: boolean } {
        const hasPositive = auras.hasActiveAura(positiveChargeName);
        const hasNegative = auras.hasActiveAura(negativeChargeName);
        const hasReversed = reversedSuffix
            ? auras.hasAnyActiveAura(positiveChargeName + reversedSuffix, negativeChargeName + reversedSuffix)
            : false;
        return { hasPositive, hasNegative, hasReversed };
    }
    /** Compares an aura's value (`getAuraValue`) with `threshold`; throws on an unknown operator. */
    checkAuraThreshold(
        auras: SelfAuras | TargetAuras, auraName: string, threshold: number,
        comparison: "<" | ">" | "<=" | ">=" | "==" | "!=" = ">=",
    ): boolean {
        const value = auras.getAuraValue(auraName);
        switch (comparison) {
            case "<": return value < threshold;
            case ">": return value > threshold;
            case "<=": return value <= threshold;
            case ">=": return value >= threshold;
            case "==": return value === threshold;
            case "!=": return value !== threshold;
            default: throw new Error(`Invalid comparison operator: ${comparison}`);
        }
    }
    /** The auras whose name matches `namePattern` without case; `*` matches anything. */
    getAurasMatchingPattern(auras: Aura[], namePattern: string): Aura[] {
        if (namePattern.includes("*")) {
            // As in C#, the rest of the pattern is a regular expression.
            const regex = new RegExp(`^${namePattern.split("*").join(".*")}$`, "i");
            return auras.filter((a) => regex.test(a.name));
        }
        return auras.filter((a) => a.name.toLowerCase() === namePattern.toLowerCase());
    }
    /** Whether every condition holds for its aura's value (`getAuraValue`, 0 when absent). */
    shouldUseSkill(auras: SelfAuras | TargetAuras, conditions: Record<string, (value: number) => boolean>): boolean {
        return Object.entries(conditions).every(([name, condition]) => condition(auras.getAuraValue(name)));
    }
}

/** `bot.kill`: kill monsters in your cell. */
export class Kill {
    /** Kills one monster in your cell (name, "*" or map ID). */
    monster(monster: string | number): void { call("kill.monster", monster); }
    player(name: string): void { call("kill.player", name); }
    /**
     * Kills monsters in your cell until you have `quantity` of `item`.
     * `monster` may be "A|B" or "*". Picks the item up from the drops.
     */
    forItem(monster: string, item: string, quantity: number, tempItem = false): void {
        call("kill.forItem", monster, item, quantity, tempItem);
    }
    forItems(monster: string, items: string[], quantities: number[], tempItems = false): void {
        call("kill.forItems", monster, items, quantities, tempItems);
    }
}

/** `bot.hunt`: like `kill`, but jumps between cells to find the monster. */
export class Hunt {
    monster(monster: string | number): void { call("hunt.monster", monster); }
    forItem(monster: string, item: string, quantity: number, tempItem = false): void {
        call("hunt.forItem", monster, item, quantity, tempItem);
    }
    forItems(monster: string, items: string[], quantities: number[], tempItems = false): void {
        call("hunt.forItems", monster, items, quantities, tempItems);
    }
}

/** `bot.monsters`: monsters in the current map. */
export class Monsters {
    /** All monsters of the map. */
    get mapMonsters(): Monster[] { return call("monsters.mapMonsters"); }
    /** Monsters in your cell. */
    get currentMonsters(): Monster[] { return call("monsters.currentMonsters"); }
    /** Monsters in your cell you can attack now. */
    get currentAvailableMonsters(): Monster[] { return call("monsters.currentAvailableMonsters"); }
    exists(monster: string | number): boolean { return call("monsters.exists", monster); }
    /** Cells with a living monster of this name. */
    getLivingMonsterCells(monster: string | number): string[] { return call("monsters.getLivingMonsterCells", monster); }
    getMonstersByCell(cell: string): Monster[] { return call("monsters.getMonstersByCell", cell); }
}

/** `bot.quests`. */
export class Quests {
    /** Quests loaded in the client (`world.questTree`). */
    get tree(): Quest[] { return call("quests.tree"); }
    get active(): Quest[] { return call("quests.active"); }
    /** Quests registered for automatic turn-in. */
    get registered(): number[] { return call("quests.registered"); }

    load(...ids: number[]): void { call("quests.load", ids); }
    /** Loads a quest and waits until the client has it. */
    ensureLoad(id: number): Quest | null { return call("quests.ensureLoad", id); }
    get(id: number): Quest | null { return call("quests.get", id); }
    accept(id: number): boolean { return call("quests.accept", id); }
    acceptMany(...ids: number[]): void { call("quests.acceptMany", ids); }
    /** Accepts, retrying up to `options.questAcceptAndCompleteTries` times. */
    ensureAccept(id: number): boolean { return call("quests.ensureAccept", id); }
    /** Turns in a quest; `itemId` picks the reward for choose-a-reward quests. */
    complete(id: number, itemId = -1, special = false): boolean { return call("quests.complete", id, itemId, special); }
    ensureComplete(id: number, itemId = -1, special = false): boolean { return call("quests.ensureComplete", id, itemId, special); }
    isInProgress(id: number): boolean { return call("quests.isInProgress", id); }
    /** The quest's requirements are in your inventory (`status == "c"`). */
    canComplete(id: number): boolean { return call("quests.canComplete", id); }
    /** Checks the requirement items yourself (inventory, temp inventory, bank). */
    canCompleteFullCheck(id: number): boolean { return call("quests.canCompleteFullCheck", id); }
    isUnlocked(id: number): boolean { return call("quests.isUnlocked", id); }
    hasBeenCompleted(id: number): boolean { return call("quests.hasBeenCompleted", id); }
    isDailyComplete(id: number): boolean { return call("quests.isDailyComplete", id); }
    isAvailable(id: number): boolean { return call("quests.isAvailable", id); }
    /** Tells the client a quest chain value changed (fixes stale quest locks). */
    updateQuest(id: number): void { call("quests.updateQuest", id); }
    /** Accepts and turns these in automatically in the background while you farm. */
    registerQuests(...ids: number[]): void { call("quests.registerQuests", ids); }
    unregisterQuests(...ids: number[]): void { call("quests.unregisterQuests", ids); }
    unregisterAll(): void { call("quests.unregisterAll"); }
}

/** VibeSkua `BoostType`. */
export enum BoostType {
    Gold = "Gold",
    Class = "Class",
    Reputation = "Reputation",
    Experience = "Experience",
}

/**
 * `bot.boosts`: uses boost items on a 30 s timer while their boost isn't active (VibeSkua
 * `Bot.Boosts`). Pick a boost with `setUses`, give it an item ID (`setBoostId`), then `start()`.
 * Ending the script stops the timer and turns every boost off.
 */
export class Boosts {
    /** The timer runs. */
    get enabled(): boolean { return call("boosts.enabled"); }
    /** Whether the timer uses `type` (VibeSkua `UseGoldBoost`, `UseClassBoost`, ...). */
    uses(type: BoostType): boolean { return call("boosts.uses", type); }
    setUses(type: BoostType, value: boolean): void { call("boosts.setUses", type, value); }
    /** The item ID the timer uses for `type` (VibeSkua `GoldBoostID`, ...); 0 for none. */
    boostId(type: BoostType): number { return call("boosts.id", type); }
    setBoostId(type: BoostType, id: number): void { call("boosts.setId", type, id); }
    /**
     * The ID of the first boost item of `type` in the inventory, else (with `searchBank`) in the
     * bank, moving it to the inventory; 0 when you have none.
     */
    getBoostId(type: BoostType, searchBank = true): number { return call("boosts.getBoostId", type, searchBank); }
    /** Finds the boost item of `type` (see {@link getBoostId}) and sets it as its ID. */
    setBoostIdFromInventory(type: BoostType, searchBank = true): void { this.setBoostId(type, this.getBoostId(type, searchBank)); }
    /** True while a boost of `type` is active on your character. */
    isActive(type: BoostType): boolean { return call("boosts.isActive", type); }
    /** Uses the boost item `id` now. */
    useBoost(id: number): void { call("boosts.useBoost", id); }
    /** Starts the timer: uses the chosen boosts at once, then every 30 s. */
    start(): void { call("boosts.start"); }
    stop(): void { call("boosts.stop"); }
}

/** `bot.drops`: the drop stack. */
export class Drops {
    /** Drops waiting in the drop stack. */
    get current(): InventoryItem[] { return call("drops.current"); }
    /** Names the pickup loop accepts. */
    get toPickup(): string[] { return call("drops.toPickup"); }
    get toPickupIds(): number[] { return call("drops.toPickupIds"); }
    get running(): boolean { return call("drops.running"); }
    get rejectElse(): boolean { return call("drops.rejectElse"); }
    /** Reject every drop the pickup loop doesn't accept. */
    set rejectElse(value: boolean) { call("drops.setRejectElse", value); }

    exists(item: string | number): boolean { return call("drops.exists", item); }
    /** Adds names or IDs to the pickup list (the loop picks them up). */
    add(...items: (string | number)[]): void { call("drops.add", items); }
    remove(...items: (string | number)[]): void { call("drops.remove", items); }
    clear(): void { call("drops.clear"); }
    /** Picks up these drops now, if they're in the drop stack. */
    pickup(...items: (string | number)[]): void { call("drops.pickup", items); }
    pickupAll(): void { call("drops.pickupAll"); }
    pickupACItems(): void { call("drops.pickupACItems"); }
    rejectAll(): void { call("drops.rejectAll"); }
    rejectExcept(...items: (string | number)[]): void { call("drops.rejectExcept", items); }
    /** Starts the background pickup loop. */
    start(): void { call("drops.start"); }
    stop(): void { call("drops.stop"); }
}

/** Shared by inventory, bank, temp inventory and house. */
export class ItemContainer {
    constructor(private readonly module: string) {}

    get items(): InventoryItem[] { return call(`${this.module}.items`); }
    get slots(): number { return call(`${this.module}.slots`); }
    get usedSlots(): number { return call(`${this.module}.usedSlots`); }
    get freeSlots(): number { return call(`${this.module}.freeSlots`); }

    /** You have at least `quantity` (a class counts at any quantity; 0 is always true). */
    contains(item: string | number, quantity = 1): boolean { return call(`${this.module}.contains`, item, quantity); }
    /** All (`needAll`) or any of the items, each at least `quantity`. */
    containsAll(items: (string | number)[], quantity = 1): boolean { return call(`${this.module}.containsMany`, items, quantity, true); }
    containsAny(items: (string | number)[], quantity = 1): boolean { return call(`${this.module}.containsMany`, items, quantity, false); }
    getItem(item: string | number): InventoryItem | null { return call(`${this.module}.getItem`, item); }
    /** 0 when you don't have it. */
    getQuantity(item: string | number): number { return call(`${this.module}.getQuantity`, item); }
    isMaxStack(item: string | number): boolean { return call(`${this.module}.isMaxStack`, item); }
    isEquipped(item: string | number): boolean { return call(`${this.module}.isEquipped`, item); }
}

/** `bot.inventory`. */
export class Inventory extends ItemContainer {
    constructor() { super("inventory"); }
    hasSpaceFor(item: string | number, quantity = 1): boolean { return call("inventory.hasSpaceFor", item, quantity); }
    equipItem(item: string | number): boolean { return call("inventory.equipItem", item); }
    equipUsableItem(item: string | number): void { call("inventory.equipUsableItem", item); }
    toBank(item: string | number): boolean { return call("inventory.toBank", item); }
    ensureToBank(item: string | number): boolean { return call("inventory.ensureToBank", item); }
}

/** `bot.bank`. */
export class Bank extends ItemContainer {
    constructor() { super("bank"); }
    /** The bank was loaded this login. */
    get loaded(): boolean { return call("bank.loaded"); }
    set loaded(value: boolean) { call("bank.setLoaded", value); }
    /** Opens the bank window. */
    open(): void { call("bank.open"); }
    /** Loads the bank's items from the server. */
    load(waitForLoad = true): void { call("bank.load", waitForLoad); }
    toInventory(item: string | number): boolean { return call("bank.toInventory", item); }
    ensureToInventory(item: string | number): boolean { return call("bank.ensureToInventory", item); }
    swap(inventoryItem: string | number, bankItem: string | number): boolean { return call("bank.swap", inventoryItem, bankItem); }
}

/** `bot.house`. */
export class House extends ItemContainer {
    constructor() { super("house"); }
    toBank(item: string | number): boolean { return call("house.toBank", item); }
    ensureToBank(item: string | number): boolean { return call("house.ensureToBank", item); }
}

/** `bot.shops`. */
export class Shops {
    get isLoaded(): boolean { return call("shops.isLoaded"); }
    get id(): number { return call("shops.id"); }
    get name(): string { return call("shops.name"); }
    get items(): ShopItem[] { return call("shops.items"); }
    load(shopId: number): void { call("shops.load", shopId); }
    /** Buys from the loaded shop by name or item ID; `quantity` -1 buys one stack's worth. */
    buyItem(item: string | number, quantity = -1, shopItemId = 0): boolean { return call("shops.buyItem", item, quantity, shopItemId); }
    sellItem(item: string | number, quantity = 1): boolean { return call("shops.sellItem", item, quantity); }
}

export enum SkillUseMode {
    UseIfAvailable = "UseIfAvailable",
    WaitForCooldown = "WaitForCooldown",
}

export enum ClassUseMode {
    Base = "Base",
    Atk = "Atk",
    Def = "Def",
    Farm = "Farm",
    Solo = "Solo",
    Supp = "Supp",
    Dodge = "Dodge",
    Ultra = "Ultra",
}

/** `bot.skills`: the skill rotation that runs while you fight. */
export class Skills {
    get timerRunning(): boolean { return call("skills.timerRunning"); }
    /** Starts the rotation saved for the equipped class (Combat > Skills) in `mode`. */
    startAdvanced(className?: string, autoEquip = true, mode: ClassUseMode = ClassUseMode.Base): void {
        call("skills.startAdvanced", className ?? null, autoEquip, mode);
    }
    /** Starts a rotation given as text, e.g. "1 | 2 | 3 | 4 H<50" (Advanced Skills syntax). */
    startSkills(skills: string, timeout = 250, mode: SkillUseMode = SkillUseMode.WaitForCooldown): void {
        call("skills.startSkills", skills, timeout, mode);
    }
    stop(): void { call("skills.stop"); }
    pause(): void { call("skills.pause"); }
    resume(): void { call("skills.resume"); }
    canUseSkill(index: number): boolean { return call("skills.canUseSkill", index); }
    useSkill(index: number): void { call("skills.useSkill", index); }
}

/** Waits. `timeout` counts tries of ~100 ms each, like VibeSkua (10 = about 1 s). */
export class Wait {
    /** Polls `condition` every `interval` ms until it is true; false after `timeout` tries. */
    forTrue(condition: () => boolean, timeout = 10, interval = 100): boolean {
        for (let i = 0; timeout <= 0 || i < timeout; i++) {
            if (condition()) return true;
            bot.sleep(interval);
        }
        return condition();
    }
    /** Waits until the game allows `action` again (`world.lock[action]`), e.g. "acceptQuest". */
    forActionCooldown(action: string, timeout = 40): boolean { return call("wait.forActionCooldown", action, timeout); }
    forMapLoad(map: string, timeout = 20): boolean { return call("wait.forMapLoad", map, timeout); }
    forCellChange(cell: string): boolean { return call("wait.forCellChange", cell); }
    forCombatExit(timeout = 10): boolean { return call("wait.forCombatExit", timeout); }
    forMonsterSpawn(monster: string | number, timeout = 10): boolean { return call("wait.forMonsterSpawn", monster, timeout); }
    forMonsterDeath(timeout = -1): boolean { return call("wait.forMonsterDeath", timeout); }
    forDrop(item: string | number, timeout = 10): boolean { return call("wait.forDrop", item, timeout); }
    forPickup(item: string | number, timeout = 10): boolean { return call("wait.forPickup", item, timeout); }
    forItemEquip(item: string | number, timeout = 10): boolean { return call("wait.forItemEquip", item, timeout); }
    forItemBuy(timeout = 10): boolean { return call("wait.forItemBuy", timeout); }
    forItemSell(timeout = 10): boolean { return call("wait.forItemSell", timeout); }
    forQuestAccept(id: number, timeout = 14): boolean { return call("wait.forQuestAccept", id, timeout); }
    forQuestComplete(id: number, timeout = 10): boolean { return call("wait.forQuestComplete", id, timeout); }
    forBankLoad(timeout = 20): boolean { return call("wait.forBankLoad", timeout); }
    forFullyRested(timeout = -1): boolean { return call("wait.forFullyRested", timeout); }
}

/** `bot.send`: raw packets. */
export class Send {
    /** Sends a packet to the server, e.g. "%xt%zm%cmd%1%tfer%...%". */
    packet(packet: string, type: "String" | "Xt" = "String"): void { call("send.packet", packet, type); }
    /** Pretends the server sent this packet to the client. */
    clientPacket(packet: string, type: "str" | "json" | "xml" = "str"): void { call("send.clientPacket", packet, type); }
    clientServer(message: string, sentBy = "Turu"): void { call("send.clientMessage", "server", message, sentBy); }
    clientModerator(message: string, sentBy = "Turu"): void { call("send.clientMessage", "moderator", message, sentBy); }
    clientWarning(message: string, sentBy = "Turu"): void { call("send.clientMessage", "warning", message, sentBy); }
    whisper(name: string, message: string): void { call("send.whisper", name, message); }
}

/** `bot.lite`: the AQW Lite client's settings (`litePreference.data`). */
export class Lite {
    get(key: string): unknown { return call("lite.get", key); }
    set(key: string, value: boolean | number | string): void { call("lite.set", key, value); }
    /** A `litePreference.data.dOptions` value, e.g. "disRed". */
    getOption(key: string): unknown { return call("lite.getOption", key); }
    set untargetSelf(v: boolean) { this.set("bUntargetSelf", v); }
    set untargetDead(v: boolean) { this.set("bUntargetDead", v); }
    set customDropsUI(v: boolean) { this.set("bCustomDrops", v); }
    set questLogTurnIns(v: boolean) { this.set("bQuestLog", v); }
    set reacceptQuest(v: boolean) { this.set("bReaccept", v); }
    set smoothBackground(v: boolean) { this.set("bSmoothBG", v); }
    set showMonsterType(v: boolean) { this.set("bMonsType", v); }
    set chatUI(v: boolean) { this.set("bChatUI", v); }
    set aurasUI(v: boolean) { this.set("bAuras", v); }
    set characterSelectScreen(v: boolean) { this.set("bCharSelect", v); }
    set visualSkillCooldowns(v: boolean) { this.set("bSkillCD", v); }
    set disableSoundFx(v: boolean) { this.set("bDisSoundFX", v); }
    set disableAuraAnimations(v: boolean) { this.set("bDisAuraAnim", v); }
    set disableDamageStrobe(v: boolean) { this.set("bDisDmgStrobe", v); }
    set disableMonsterAnimation(v: boolean) { this.set("bDisMonAnim", v); }
    set disableSelfAnimation(v: boolean) { this.set("bDisSelfMAnim", v); }
    set disableSkillAnimation(v: boolean) { this.set("bDisSkillAnim", v); }
    set disableWeaponAnimation(v: boolean) { this.set("bDisWepAnim", v); }
    set freezeMonsterPosition(v: boolean) { this.set("bFreezeMons", v); }
    set disableRedWarning(v: boolean) { call("lite.setOption", "disRed", v); }
    set quantityWarnings(v: boolean) { call("lite.setOption", "termsAgree", v); }
}

/** The kind of a script option: a checkbox, a whole number, a text box, or a list of names. */
export type ScriptOptionKind = "bool" | "int" | "text" | "choice";

/** One of a script's options (VibeSkua `Option<T>`), as the Script Options window shows it. */
export interface ScriptOption {
    /** The name `bot.config.get` reads (VibeSkua `Name`). */
    name: string;
    /** The label in the window (VibeSkua `DisplayName`). */
    displayName: string;
    /** The text under the label (VibeSkua `Description`). */
    description: string;
    kind: ScriptOptionKind;
    /** The value until the user saves another one (VibeSkua `DefaultValue`). */
    defaultValue: boolean | number | string;
    /** For a `"choice"`: the names it can take (VibeSkua: the enum's members). */
    choices?: string[];
}

/** A named list of options (one entry of VibeSkua's `MultiOptions`). */
export interface ScriptOptionGroup {
    /** The group's name: `bot.config.get(name, option)` and the `Name:` prefix in the options file. */
    name: string;
    options: ScriptOption[];
}

/** A script's options: the file the values are saved in, and the options (see {@link defineScriptOptions}). */
export interface ScriptOptions {
    /** The saved values go to `options/<storage>.cfg`, as in VibeSkua (`OptionsStorage`). */
    storage: string;
    options: ScriptOption[];
    /** Option groups (VibeSkua `MultiOptions`), after `options`. See {@link defineScriptOptions}. */
    groups?: ScriptOptionGroup[];
    /**
     * Don't open the Script Options window before each run (VibeSkua `DontPreconfigure`). Set it
     * when the script opens the window itself, as CoreBots does with `Core.skipOptions`.
     */
    dontPreconfigure?: boolean;
}

/** Makers for {@link ScriptOption}s, named as VibeSkua's `Option<T>` types. */
export const option = {
    /** A checkbox. */
    bool(name: string, displayName: string, description: string, defaultValue = false): ScriptOption {
        return { name, displayName, description, kind: "bool", defaultValue };
    },
    /** A whole number. */
    int(name: string, displayName: string, description: string, defaultValue = 0): ScriptOption {
        return { name, displayName, description, kind: "int", defaultValue };
    },
    /** A text box. */
    text(name: string, displayName: string, description: string, defaultValue = ""): ScriptOption {
        return { name, displayName, description, kind: "text", defaultValue };
    },
    /**
     * One of a list of names, picked from a drop-down (VibeSkua's enum options, `Option<MyEnum>`).
     * The window shows `_` as spaces; `bot.config.get` returns the name. Pass the members of a
     * TypeScript enum with `choices(MyEnum)`. The default is the first name when not given.
     *
     * @example
     * enum PetChoice { None = "None", Dragon_Pet = "Dragon_Pet" }
     * option.choice("PetChoice", "Choose Your Pet", "", choices(PetChoice), PetChoice.None)
     * // later: bot.config.get<PetChoice>("PetChoice")
     */
    choice(name: string, displayName: string, description: string, choices: readonly string[], defaultValue?: string): ScriptOption {
        return { name, displayName, description, kind: "choice", defaultValue: defaultValue ?? choices[0] ?? "", choices: [...choices] };
    },
};

/**
 * The member names of a TypeScript enum, in order, for {@link option.choice}. For a string enum
 * whose values are its names (`enum Mode { all = "all" }`), these are also its values.
 */
export function choices(enumObject: Record<string, string | number>): string[] {
    // A numeric enum maps its values back to its names too: keep the names only.
    return Object.keys(enumObject).filter((key) => !/^\d+$/.test(key));
}

/**
 * Declares a script's options (VibeSkua `OptionsStorage` and `Options`). Export the result as
 * `scriptOptions`: the Script Options window (Scripts > Edit Script Options) shows the options
 * once the script is loaded, and `bot.config.get` reads the values. Start opens the window and
 * waits for it to close, unless `dontPreconfigure` is true.
 *
 * `groups` are VibeSkua's `MultiOptions`: each key is a group, shown after `options` under its
 * name (with `_` as spaces) and saved as `Group:Name=Value`. Read them with
 * `bot.config.get(group, name)`.
 *
 * @example
 * export const scriptOptions = defineScriptOptions("MyScript", [
 *     option.int("PotionQuant", "Potion Quantity", "Desired stack amount", 300),
 * ]);
 * // With groups (VibeSkua `MultiOptions = { "Generic", "Select" }`):
 * export const scriptOptions = defineScriptOptions("MergeStorage", [], true, {
 *     Generic: [option.choice("mode", "Mode", "", ["all", "acOnly", "mergeMats", "select"])],
 *     Select: [option.bool("1234", "Some Item", "")],
 * });
 */
export function defineScriptOptions(
    storage: string,
    options: ScriptOption[],
    dontPreconfigure = false,
    groups: Record<string, ScriptOption[]> = {},
): ScriptOptions {
    return { storage, options, dontPreconfigure, groups: Object.entries(groups).map(([name, list]) => ({ name, options: list })) };
}

/** `bot.config`: the values of the script's options (VibeSkua `Bot.Config`). */
export class Config {
    /**
     * The value of an option: the one the user saved in the Script Options window, or its
     * default. Throws for a name the script doesn't declare.
     */
    get<T extends boolean | number | string = boolean | number | string>(name: string): T;
    /** The value of an option of a group (VibeSkua `Config.Get<T>(category, name)`). */
    get<T extends boolean | number | string = boolean | number | string>(group: string, name: string): T;
    get(...args: string[]): unknown { return call("config.get", ...args); }
    /** Sets an option and saves it to the script's options file (VibeSkua `Config.Set`). */
    set(name: string, value: boolean | number | string): void;
    /** Sets an option of a group and saves it (VibeSkua `Config.Set(category, name, value)`). */
    set(group: string, name: string, value: boolean | number | string): void;
    set(...args: (boolean | number | string)[]): void { call("config.set", ...args); }
    /** Whether the script declares the option (VibeSkua `Config.Options.Contains`). */
    has(name: string): boolean;
    /** Whether the script declares the option in that group. */
    has(group: string, name: string): boolean;
    has(...args: string[]): boolean { return call("config.has", ...args); }
    /**
     * Opens the Script Options window and waits until the user saves or cancels it (VibeSkua
     * `Config.Configure()`). Returns at once when the script declares no options, or nobody can
     * answer it: `--headless`, and runs started from the command line, the HTTP API or auto-relogin.
     */
    configure(): void { call("config.configure"); }
}

/** The bot's options (Options > Game). Changes apply at once and last until the script stops. */
export interface OptionValues {
    lagKiller: boolean;
    hidePlayers: boolean;
    disableFx: boolean;
    disableCollisions: boolean;
    showFps: boolean;
    setFps: number;
    aggroMonsters: boolean;
    aggroAllMonsters: boolean;
    attackWithoutTarget: boolean;
    infiniteRange: boolean;
    magnetise: boolean;
    huntDelay: number;
    huntBuffer: number;
    restPackets: boolean;
    safeTimings: boolean;
    acceptACDrops: boolean;
    acceptAllDrops: boolean;
    rejectAllDrops: boolean;
    reloginServer: string;
    autoRelogin: boolean;
    autoReloginAny: boolean;
    retryRelogin: boolean;
    safeRelogin: boolean;
    reloginTries: number;
    reloginTryDelay: number;
    loginTimeout: number;
    /** Start auto-relogin on an unexpected disconnect or a lost connection (VibeSkua's "Relogin Watchdog"). */
    reloginWatchdog: boolean;
    privateRooms: boolean;
    privateNumber: number;
    skipCutscenes: boolean;
    headlessMode: boolean;
    streamerMode: boolean;
    customName: string;
    customGuild: string;
    walkSpeed: number;
    actionDelay: number;
    maximumTries: number;
    joinMapTries: number;
    questAcceptAndCompleteTries: number;
    loadTimeout: number;
    disableDeathAds: boolean;
}

export type Options = OptionValues & {
    get<K extends keyof OptionValues>(name: K): OptionValues[K];
    set<K extends keyof OptionValues>(name: K, value: OptionValues[K]): void;
};

function makeOptions(): Options {
    const target = {
        get: (name: string) => call("options.get", name),
        set: (name: string, value: unknown) => call("options.set", name, value),
    };
    return new Proxy(target, {
        get: (t, name) => (name === "get" || name === "set" ? t[name] : call("options.get", String(name))),
        set: (_t, name, value) => {
            call("options.set", String(name), value);
            return true;
        },
    }) as unknown as Options;
}

export interface EventMap {
    gameLoaded: null;
    logout: null;
    playerDeath: null;
    monsterKilled: { mapId: number };
    questAccepted: { questId: number };
    questTurnedIn: { questId: number };
    mapChanged: { map: string };
    cellChanged: { map: string; cell: string; pad: string };
    itemDropped: InventoryItem;
    /** An item arrived in the inventory or temp inventory (pickup, quest reward, purchase). */
    itemAdded: InventoryItem & { quantityNow: number };
    itemAddedToBank: InventoryItem & { quantityNow: number };
    tryBuyItem: { shopId: number; itemId: number; shopItemId: number };
    login: { username: string };
    /** A boss prepares a counter attack (`faded` false) or it ended (`faded` true). */
    counterAttack: { faded: boolean };
    playerAFK: null;
    runToArea: { zone: string };
    itemBought: null;
    itemSold: null;
    bankLoaded: null;
    shopLoaded: null;
    connectionLost: null;
    /** Auto-relogin started; `wasKicked` when the login screen showed the kick warning. */
    reloginTriggered: { wasKicked: boolean };
    /** A server packet: `{ params: { type: "json" | "str", dataObj } }`. */
    extensionPacket: any;
    /** A client packet, "%xt%zm%...%". */
    packetSent: string;
    /**
     * The script is stopping: Stop was pressed, the script stopped itself (`bot.stop()`) or it
     * failed (the data is then the error message; otherwise null). Not sent when `main` simply
     * returns, like VibeSkua's `ScriptStopping`. Handlers may use the bot (Stop is reset while
     * they run); pressing Stop again interrupts them.
     */
    scriptStopping: string | null;
}

type Listener<K extends keyof EventMap> = (data: EventMap[K]) => void;
const listeners = new Map<string, Set<(data: unknown) => void>>();
let dispatching = false;

function dispatchEvents(): void {
    if (dispatching || listeners.size === 0) return;
    dispatching = true;
    try {
        const events = JSON.parse(__turuEvents()) as { name: string; data: unknown }[];
        for (const event of events) {
            for (const listener of listeners.get(event.name) ?? []) listener(event.data);
        }
    } finally {
        dispatching = false;
    }
}

// Called by the engine once the script has stopped or failed (see `EventMap.scriptStopping`).
(globalThis as any).__turuScriptStopping = (error?: string | null): void => {
    for (const listener of [...(listeners.get("scriptStopping") ?? [])]) listener(error ?? null);
};

/**
 * `bot.events`: game events. Handlers run on the script's thread, between bot calls (so a
 * script that calls the bot, or sleeps, receives them promptly).
 */
export class Events {
    /** Adds a handler; returns a function that removes it. */
    on<K extends keyof EventMap>(name: K, listener: Listener<K>): () => void {
        let set = listeners.get(name);
        if (!set) listeners.set(name, (set = new Set()));
        set.add(listener as (data: unknown) => void);
        return () => this.off(name, listener);
    }
    off<K extends keyof EventMap>(name: K, listener: Listener<K>): void {
        listeners.get(name)?.delete(listener as (data: unknown) => void);
    }
    clear(): void { listeners.clear(); }
}

/** Raw game access for things the API doesn't cover. Paths are rooted at the game. */
export class Flash {
    get<T = unknown>(path: string): T { return call("flash.get", path); }
    getStatic<T = unknown>(path: string): T { return call("flash.getStatic", path); }
    /**
     * Sets a game value (VibeSkua `Flash.SetGameObject`). Arrays and objects reach the game as AS3
     * arrays and objects: `bot.flash.set("world.questTree", {})` empties the quest tree.
     */
    set(path: string, value: unknown): void { call("flash.set", path, value); }
    isNull(path: string): boolean { return call("flash.isNull", path); }
    callGameFunction<T = unknown>(path: string, ...args: unknown[]): T { return call("flash.callGameFunction", path, ...args); }
    /** Calls one of skua.swf's ExternalInterface callbacks and returns its raw string. */
    call(name: string, ...args: unknown[]): string { return call("flash.call", name, ...args); }
}

// ---------------------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------------------

/** Longest slice of a `bot.sleep` before the handlers get a turn. */
const SLEEP_SLICE_MS = 100;

/** The functions of `bot.handlers`. The engine keeps their timing (bot/src/api/handlers.rs). */
const handlerRuns = new Map<string, () => unknown>();
/** Handlers the engine named as due and that haven't run yet. */
const dueHandlers: string[] = [];
let runningHandlers = false;

/** Runs the handlers the engine named as due. Bot calls made by a handler queue the rest. */
function runDueHandlers(names: string[]): void {
    dueHandlers.push(...names);
    if (runningHandlers) return;
    runningHandlers = true;
    try {
        for (let name = dueHandlers.shift(); name !== undefined; name = dueHandlers.shift()) {
            runHandler(name);
        }
    } finally {
        runningHandlers = false;
        dueHandlers.length = 0;
    }
}

function runHandler(name: string): void {
    const run = handlerRuns.get(name);
    if (run) {
        try {
            if (run() === false) bot.handlers.remove(name);
        } catch (error) {
            if (error instanceof ScriptStopped) throw error;
            bot.log(`Handler "${name}" failed: ${error instanceof Error ? error.message : String(error)}`);
        }
    }
    if (!call<boolean>("handlers.finish", name)) handlerRuns.delete(name);
}

/**
 * `bot.handlers`: functions that run while your script makes bot calls, like VibeSkua's
 * `Bot.Handlers`. They run between your bot calls and during `bot.sleep`, never inside another
 * handler. They don't run while the engine is inside one long wait (such as
 * `bot.wait.forMapLoad`); they run at the next call after it.
 */
export class Handlers {
    /**
     * Runs `run` every `intervalMs`. Registering a `name` that's already in use replaces that
     * handler. Returning `false` from `run` removes it. Intervals under 20 ms run at 20 ms.
     */
    register(name: string, intervalMs: number, run: () => unknown): void {
        handlerRuns.set(name, run);
        call("handlers.add", name, intervalMs, false);
    }

    /** Runs `run` once, `delayMs` from now. */
    once(name: string, delayMs: number, run: () => unknown): void {
        handlerRuns.set(name, run);
        call("handlers.add", name, delayMs, true);
    }

    /** Removes the handler; false when there was none. */
    remove(name: string): boolean {
        handlerRuns.delete(name);
        return call("handlers.remove", name);
    }

    /** Removes every handler. */
    clear(): void {
        handlerRuns.clear();
        call("handlers.clear");
    }
}

// ---------------------------------------------------------------------------------------
// The bot
// ---------------------------------------------------------------------------------------

export class Bot {
    readonly player = new Player();
    readonly map = new GameMap();
    readonly combat = new Combat();
    /** The counter-attack pause and aura checks for ultra bosses. */
    readonly ultraBossHelper = new UltraBossHelper();
    readonly kill = new Kill();
    readonly hunt = new Hunt();
    readonly monsters = new Monsters();
    readonly quests = new Quests();
    /** Your factions and ranks (VibeSkua `Bot.Reputation`). */
    readonly reputation = new Reputation();
    /** The auras on you (VibeSkua `Bot.Self`). */
    readonly self = new SelfAuras();
    /** The auras on your target (VibeSkua `Bot.Target`). */
    readonly target = new TargetAuras();
    readonly drops = new Drops();
    readonly boosts = new Boosts();
    readonly inventory = new Inventory();
    readonly bank = new Bank();
    readonly tempInv = new ItemContainer("tempInv");
    readonly house = new House();
    readonly shops = new Shops();
    readonly skills = new Skills();
    readonly wait = new Wait();
    readonly send = new Send();
    readonly servers = new Servers();
    readonly lite = new Lite();
    readonly config = new Config();
    readonly options: Options = makeOptions();
    readonly events = new Events();
    readonly handlers = new Handlers();
    readonly flash = new Flash();

    /** Writes a line to the script log. */
    log(message: unknown): void { call("log", typeof message === "string" ? message : JSON.stringify(message)); }
    /**
     * Sleeps; throws `ScriptStopped` early if the script is stopped. The sleep is cut into
     * slices so that `bot.handlers` run during it.
     */
    sleep(ms: number): void {
        let left = ms;
        do {
            const slice = Math.min(Math.max(left, 0), SLEEP_SLICE_MS);
            call("sleep", slice);
            left -= slice;
        } while (left > 0);
    }
    /** True once Stop was pressed. */
    get shouldExit(): boolean { return call("shouldExit"); }
    /**
     * The lines of your CoreBots Options file (Options > CoreBots, saved per character as
     * `options/CBO_Storage(<username>).txt`), e.g. `["PrivateRooms: True", ...]`; null when
     * you aren't logged in or haven't saved one. CoreBots reads it in `setOptions()`.
     */
    coreBotsOptions(): string[] | null { return call("coreBotsOptions"); }
    /** Stops the script (throws `ScriptStopped`). */
    stop(): never {
        call("stop");
        throw new ScriptStopped();
    }
}

export const bot = new Bot();

// `console.log` goes to the script log too.
(globalThis as any).console = {
    log: (...args: unknown[]) => bot.log(args.map(String).join(" ")),
    warn: (...args: unknown[]) => bot.log("[warn] " + args.map(String).join(" ")),
    error: (...args: unknown[]) => bot.log("[error] " + args.map(String).join(" ")),
};
