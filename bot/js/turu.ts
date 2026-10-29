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
    const result = JSON.parse(raw) as { ok?: T; error?: string; stopped?: boolean };
    if (result.stopped) throw new ScriptStopped();
    if (result.error !== undefined) throw new BotError(`${method}: ${result.error}`);
    dispatchEvents();
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
    equipped: boolean;
    /** Member-only item. */
    upgrade: boolean;
    /** Costs AC. */
    coins: boolean;
    temp: boolean;
    house: boolean;
    /** "Class", "Sword", "Item", "Resource", "Quest Item", ... */
    category: string;
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

export interface Server {
    name: string;
    ip: string;
    port: number;
    online: boolean;
    upgrade: boolean;
    playerCount: number;
    maxPlayers: number;
    language: string;
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
    get cell(): string { return call("player.cell"); }
    get pad(): string { return call("player.pad"); }
    get level(): number { return call("player.level"); }
    get health(): number { return call("player.health"); }
    get maxHealth(): number { return call("player.maxHealth"); }
    get mana(): number { return call("player.mana"); }
    get maxMana(): number { return call("player.maxMana"); }
    get gold(): number { return call("player.gold"); }
    get isMember(): boolean { return call("player.isMember"); }
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
    loginTimeout: number;
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
    /** A server packet: `{ params: { type: "json" | "str", dataObj } }`. */
    extensionPacket: any;
    /** A client packet, "%xt%zm%...%". */
    packetSent: string;
    /** The script is stopping (Stop pressed, finished or failed). */
    scriptStopping: null;
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
    set(path: string, value: boolean | number | string | null): void { call("flash.set", path, value); }
    isNull(path: string): boolean { return call("flash.isNull", path); }
    callGameFunction<T = unknown>(path: string, ...args: unknown[]): T { return call("flash.callGameFunction", path, ...args); }
    /** Calls one of skua.swf's ExternalInterface callbacks and returns its raw string. */
    call(name: string, ...args: unknown[]): string { return call("flash.call", name, ...args); }
}

// ---------------------------------------------------------------------------------------
// The bot
// ---------------------------------------------------------------------------------------

export class Bot {
    readonly player = new Player();
    readonly map = new GameMap();
    readonly combat = new Combat();
    readonly kill = new Kill();
    readonly hunt = new Hunt();
    readonly monsters = new Monsters();
    readonly quests = new Quests();
    readonly drops = new Drops();
    readonly inventory = new Inventory();
    readonly bank = new Bank();
    readonly tempInv = new ItemContainer("tempInv");
    readonly house = new House();
    readonly shops = new Shops();
    readonly skills = new Skills();
    readonly wait = new Wait();
    readonly send = new Send();
    readonly lite = new Lite();
    readonly options: Options = makeOptions();
    readonly events = new Events();
    readonly flash = new Flash();

    /** Writes a line to the script log. */
    log(message: unknown): void { call("log", typeof message === "string" ? message : JSON.stringify(message)); }
    /** Sleeps; throws `ScriptStopped` early if the script is stopped. */
    sleep(ms: number): void { call("sleep", ms); }
    /** True once Stop was pressed. */
    get shouldExit(): boolean { return call("shouldExit"); }
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
