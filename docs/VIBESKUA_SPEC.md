# VibeSkua survey (spec for Rust/Ruffle/egui/TS reimplementation)

## 0. Layout and facts that matter most
- The "Skua fork" is a Windows-only .NET app. The Flash player is the ActiveX Flash OCX (CleanFlash), not Ruffle. Solution projects:
  - `Skua.App.Avalonia`: main client.
  - `Skua.Manager.Avalonia`: launcher and account manager.
  - `Skua.Core`: script API implementations, compiler, proxy.
  - `Skua.Core.Interfaces`, `Skua.Core.Models`, `Skua.Core.Utils`.
  - `Skua.Core.Generators`: Roslyn source generators that produce the Flash bindings.
  - `Skua.Shared.Avalonia`: theme and shared controls.
  - `Skua.AS3`: ActionScript sources.
  - `Skua.Plugin.ButlerV5`, `Skua.Plugin.Cosmetics`, `Skua.Plugin.DailyTracker`.
  - `FFDec/`: JPEXS decompiler, used for map-item scraping.
- Scripts are C# files with `public void ScriptMain(IScriptInterface Bot)`. Examples are in `/Users/aryamuktiaraafi/projects/Scripts` (`CoreBots.cs`, `CoreFarms.cs`, and others).
- The Rust port needs to reproduce the Flash-side API, which is plain AS3 reading and writing private game objects by dotted path. That is feasible in Ruffle only if Ruffle runs the official `Game.swf` and can run `skua.swf`'s loader logic. Ruffle must support `Loader.load` of a remote SWF, `ExternalInterface`, `ApplicationDomain.getDefinition`, `Security.loadPolicyFile`, `XMLSocket`, and `sfc` socket access. Alternatively, reimplement the `skua.swf` host logic natively against Ruffle's AVM2 object model.

## 1. SWF loading

### SWF files
- `R/Skua.AS3/skua/bin/skua.swf` (24,960 bytes) is the build output.
- `R/Skua.App.Avalonia/skua.swf` (22,884 bytes) is a committed older copy. The csproj line `<None Include="..\Skua.AS3\skua\bin\skua.swf" Link="skua.swf" Condition="Exists(...)">` copies the bin version to the output dir at build time. The two files differ, so the committed app copy is stale.
- No game SWF is bundled. `skua.swf` is a thin host (`skua.Main`) that downloads the real AQW client at runtime.
- `QuestData.json` (19 MB) and `AdvancedSkills.json` (151 KB) are bundled in `Skua.App.Avalonia/`.

### Build
- `Skua.AS3/Build.bat` runs `compile-as3.ps1`.
- The script calls Flex `mxmlc -source-path skua/src -default-size 958 550 -target-player 28.0 -optimize skua/src/skua/Main.as`.
- Main class metadata: `[SWF(frameRate="30", backgroundColor="#000000", width="958", height="550")]`.

### Hosting
- `Skua.App.Avalonia/Flash/FlashUtil.cs`, compiled under `#if IS_WINDOWS`, wraps `AxShockwaveFlashObjects.AxShockwaveFlash`. This is an ActiveX control, so it needs a CleanFlash install.
- The `Controls/WinFormsFlashHost` class is an Avalonia `NativeControlHost` that embeds the WinForms control.
- The OCX is configured with `WMode="window"`, `ScaleMode=0`, `CtlScale="ShowAll"`.
- The SWF bytes are loaded in memory (`PreloadSwf()` caches them) and fed through `flash.OcxState = new AxHost.State(stream,1,false,null)` with a header of `int(8+len)`, `1432769894` ("fUfU"), `len`, then the bytes.
- `EoLHook` uses CoreHook to hook kernel32 `GetSystemTime` and force `wYear=2020`, bypassing the Flash end-of-life kill switch.
- `FlashTrustManager` writes `%APPDATA%\Macromedia\Flash Player\#Security\FlashPlayerTrust\VibeSkua.cfg`, listing the app dir, `%APPDATA%\VibeSkua`, and `%APPDATA%\Skua`.
- Headless mode sets `stage.frameRate=2`, `stage.quality="LOW"`, and mutes the mixer.
- "Clear Flash Cache" deletes `%APPDATA%\Adobe\Flash Player` and `Macromedia\Flash Player`.
- Application option "Client Animation Frame-rate" defaults to 60 (`Client.AnimationFrameRate`). It requires a restart.

### What `skua.Main` loads (`R/Skua.AS3/skua/src/skua/Main.as`)
1. `sURL = 'https://game.aq.com/game/'`.
2. `versionUrl = sURL + 'api/data/gameversion'`, fetched with `URLLoader`. The reply is JSON with `sFile`.
3. `loginURL = sURL + 'api/login/now'`.
4. The game is loaded from `sURL + 'gamefiles/' + vars.sFile + '?ver=' + Math.random()` using `flash.display.Loader`, with no LoaderContext, so it shares the app domain.
5. `Security.allowDomain('*')` is called first.
6. In `onComplete`, `stage.removeChildAt(0)` removes the skua placeholder and `game = stage.addChild(loader.content)` adds the game. The stage is set to `scaleMode=SHOW_ALL`, `align=TOP`.
7. Every key of `root.loaderInfo.parameters` is copied into `game.params[...]`.
8. The following are set on `game.params`:
   - `vars` (the gameversion JSON)
   - `sURL`
   - `sBG` (default `'hideme.swf'`, overridable)
   - `sTitle = '<font color="#FDAF2D">AURAS!!!</font>'`
   - `isEU`
   - `loginURL`
9. Listeners attached to the game:
   - `MouseEvent.CLICK` on the game.
   - `game.sfc` (the SmartFox client) gets `SFSEvent.onExtensionResponse` and `onConnectionLost`.
   - `ENTER_FRAME` handlers for `Modules.handleFrame` (throttled to 100 ms), `monitorLoginScreen`, and `monitorServerList`.
   - A `KEY_DOWN` handler for the Bank hotkey (`litePreference.data.keys['Bank']` calls `world.toggleBank()`).
10. After setup it calls `Auras.initialize()` (a 500 ms poll until `world.uoTree` and `world.monTree` exist, then a 5 s cleanup timer) and `external.call('loaded')`.
11. `Main.catchPackets()` sets `game.sfc.debug = true` and hooks `SFSEvent.onDebugMessage`. That is how raw packets are observed. It re-asserts each frame because the game's own packet logger turns debug off.
12. `Main.String.prototype.trim` is patched.
13. A custom login background is supported (`setBackgroundValues(sBG, customBackgroundURL)` loads an SWF into `mcLogin.mcTitle`). The same SWF is also loaded and added at game child index 0 as the "lag killer" background, shown when `killLag(true)`.

### Server list and other URLs
- Server list: `http://content.aq.com/game/api/data/servers` (`ScriptServers.GetServers`). It is a JSON array with fields `sName`, `sIP`, `iChat`, `iPort`, `bOnline`, `sLang`, `iCount`, `bUpg`, `iMax`, `iLevel`.
- Map SWFs: `https://game.aq.com/game/gamefiles/maps/{FilePath}`.
- Game server TCP port is typically 5588. `CaptureProxy.DefaultPort = 5588`.

### AS3 source layout (`R/Skua.AS3/skua/src/skua/`)
- `Main.as` (556 lines): loader, game access primitives, packets, injection.
- `Externalizer.as` (159 lines): registers every `ExternalInterface` callback.
- `api/`:
  - `Player.as`
  - `World.as`
  - `Monster.as`
  - `Combat.as`
  - `Skills.as`
  - `Inventory.as`
  - `Shop.as`
  - `Server.as`
  - `Auras.as`
- `module/` (frame-polled toggles):
  - `Modules.as` (registry)
  - `Module.as`
  - `HidePlayers.as`
  - `DisableFX.as`
  - `DisableCollisions.as`
  - `QuestItemRates.as` (always on)
  - `QuestRequirementWiki.as`
  - `ModalMC.as`
  - `QtySelectorMC.as`
- `remote/`:
  - `RemoteRegistry.as`
  - `LinkedObject.as`
  - `FunctionCaller.as`
  - `ObjectParent.as`
- `util/SFSEvent.as`.
- Modules registered: QuestRequirementWiki, QuestItemRates, HidePlayers, DisableCollisions, DisableFX. C# also references `ModuleBinding("OptimizePlayers")`, but that module does not exist in AS3 and `modEnable` is silently a no-op for it.

## 2. Flash <-> C# bridge

### Transport
- C# calls AS3 through the OCX's `CallFunction(xml)`.
- AS3 calls C# through `ExternalInterface.call(name, args)`, which the OCX raises as `FlashCall(request)`.
- `Externalizer.call(name, ...rest)` does `ExternalInterface.call(name, rest)`. The args are passed as one Array, but because the Flash XML serializer wraps the call into `<invoke name=...><arguments>...`, the C# `CallHandler` does `el.Elements().Select(FromFlashXml)`.
- The array arrives as a single `<array>` element, and C# converts it to `object[]` via the `"array"` case. Handlers therefore receive `args[0]` as the first rest-arg only if the wrapper array is unwrapped. In practice `HandleFlashCall` uses `args[0]` as the first rest argument, because `FromFlashXml` for the top-level call flattens the first-level arguments. Verify this against the OCX when reimplementing.

### C# to Flash (`FlashUtil.Call`)
Request string:
`<invoke name="{fn}" returntype="xml"><arguments>{args}</arguments></invoke>`

The `<arguments>` element is omitted when there are no args.

Argument encoding (`ToFlashXml`):

| .NET value | XML |
|---|---|
| null | `<null/>` |
| bool | `<true/>` or `<false/>` |
| int/long/float/double | `<number>N</number>` |
| ExpandoObject | `<object><property id="k">...</property></object>` |
| Array | `<array><property id="0">...</property></array>` |
| anything else | `<string>escaped</string>` (`SecurityElement.Escape`) |

Return handling:
- The response is parsed as XElement and `el.FirstNode.ToString()` is converted with `Convert.ChangeType(.., typeof(T))`.
- Because the response is `<string>...</string>`, the first node is the text content.
- Almost every AS3 callback therefore returns a String: `JSON.stringify(...)`, or `"true"`/`"false"` via `.toString()`.
- C# deserializes with Newtonsoft (`GetGameObject<T>`).
- Booleans come back as the text `"True"`/`"true"`, which `Convert.ChangeType` handles.
- COM transient errors (`0x8001010A`, `0x80010001`) are retried twice with 10 ms and 20 ms sleeps.
- Calls from the "Script Thread" throw `OperationCanceledException` first if the script has been asked to stop. This is the cancellation hook.
- Any call exception sends `FlashErrorMessage` (except for `isKicked`).

### Flash to C# (`CallHandler`)
- `<invoke name="fn" returntype="xml"><arguments>...</arguments></invoke>` is parsed.
- `FromFlashXml` rules: `number` becomes int or float, `true`/`false` become bool, `null` becomes null, `array` becomes `object[]`, `object` becomes an `ExpandoObject` (children are `<property id="..">`), and anything else becomes the string.
- The result is dispatched to the `FlashCall` event as `(function, args)`.

### Every ExternalInterface callback (`Externalizer.init`)
The AS3 function is `Main.*`, `Server.*`, etc. All values are Strings unless noted.

**Core**
- `loadClient()`: runs `Main.loadGame`, which calls `onAddedToStage` (fetches version JSON, then loads the game) and then `external.call('pre-load')`.
- `setBackgroundValues(sBG, customBackgroundPath)`.
- `isTrue()` returns `"true"`.
- `isNull(path)` returns `"true"` or `"false"` (`"true"` on exception).

**Game object access** (paths are dotted, rooted at `Main.instance.game`)
- `getGameObject(path)` returns `JSON.stringify(value)`.
- `getGameObjectS(path)` is the same but rooted at the game CLASS, via `gameDomain.getDefinition(getQualifiedClassName(game))`. It reads statics such as `serverIP`, `loginInfo.strUsername`, `loginInfo.strPassword`, `objLogin`.
- `getGameObjectKey(path, key)` returns `JSON.stringify(obj[key])`. It is used for paths like `litePreference.data.dOptions["debugPacket"]`. C# splits the path on `[`/`"`/`]`.
- `setGameObject(path, value)` has no return value (void). `setGameObjectKey(path, key, value)` is the keyed form.
- `getArrayObject(path, index)` returns JSON of `arr[index]`. `setArrayObject(path, index, value)`.
- `callGameFunction(path, ...args)` returns `JSON.stringify(fn.apply(null,args))`.
- `callGameFunction0(path)` is the zero-argument form.
- `selectArrayObjects(path, selector)` returns a JSON array of `_getObjectS(arr[i], selector)`.

**Server and login**
- `connectToServer(serverJson)`: this is the direct-connect path.
  1. `resetConnectionState()`.
  2. `Security.loadPolicyFile("xmlsocket://ip:port")`.
  3. Waits up to 5 s (Timer 100 ms x 50) for `objLogin`.
  4. Runs the same checks the stock UI does:
    - offline: `MsgBox.notify("Server currently offline!")`
    - full: `"Server is Full!"`
    - chat restrictions
    - member server: `ModalMC` dual dialog
    - testing server (iMax odd): dual dialog
    - email status
  5. Otherwise sets `game.objServerInfo`, `chatF.iChat`, `killLoginModals()`, then `game.connectTo(sIP, iPort)`.
- `clickServer(name)`: kills modals, loads policy files, then finds the child of `mcLogin.sl.iList` whose `tName.ti.text` contains the name and dispatches a CLICK on it. Returns `"true"` or `"false"`.
- `loadPolicyFile(ip, port)` and `loadPolicyFiles()` (iterates `serialCmd.servers` and the `iList` children).
- `killLoginModals()` closes and removes every modal stack child (`mcLogin.ModalStack`, `mcO.ModalStack`, `ui.ModalStack`, `ui.mcPopup`), calls `MsgBox.fClose()`, hides `mcLogin.warning`, and resets `serialCmdMode`/`connectingServer`.
- `resetConnectionState()`: stops the timer, kills modals, calls `sfc.disconnect()`, sets `connectingServer=false`.
- `getActiveModalMessage()` returns the text of the first modal (mcO, mcLogin, MsgBox, or the warning).

**Player**
- `isLoggedIn()` is `game!=null && game.sfc!=null && game.sfc.isConnected`.
- `isKicked()` is `mcLogin.warning.visible`.
- `walkTo(x, y, speed)` calls `pMC.walkTo` and `world.moveRequest({mc, tx, ty, sp})`. Speed 8 is replaced by `world.WALKSPEED`.
- `untargetSelf()` calls `world.cancelTarget()` if the target is the avatar.
- `attackPlayer(name)` uses `getAvatarByUserName`, then `setTarget`, then `approachTarget`.
- `getAvatar(id)` returns the JSON of `world.avatars[id].objData`.
- `getLoadouts()` returns the JSON of `world.objInfo["customs"].loadouts`.
- `gender()` returns `"\"MALE\""` or `"\"FEMALE\""` (quoted).
- `rejectExcept(csvLowercaseNames)`:
  - If `litePreference.data.bCustomDrops` is set, it iterates `cDropsUI.invTree.concat()` and calls `cDropsUI.onBtNo(item)` for items not in the list.
  - Otherwise it walks `ui.dropStack` children whose class contains `DFrame2MC`, parses the `"Name xN"` text with `/(.*)\s+x\s*(\d*)/`, and dispatches CLICK on `cnt.nbtn`.
  - Returns `"true"`.

**World**
- `jumpCorrectRoom(cell, pad, autoCorrect=true, clientOnly=false)` calls `world.moveToCell(cell, pad, clientOnly)`.
  - With autoCorrect, a 50 ms x 40 timer checks that the map is on the cell label and not playing.
  - It then checks the `Pad_\d+` children of `world.map` and re-issues `moveToCell` with a valid pad (preferred pad, then "Left", then the first).
- `disableDeathAd(bool)` sets `userPreference.data.bDeathAd = !enable`.
- `skipCutscenes()` clears `mcExtSWF` children and calls `showInterface()`.
- `hidePlayers(bool)`.
- `disableFX(bool)` nulls `avatar.rootClass.spFX`.
- `killLag(bool)` sets `world.visible = !enable`, and shows the custom lag-killer background.

**Monster**
- `availableMonsters()` returns JSON of monsters in the current cell (`world.getMonstersByCell(world.strFrame)` filtered to those with a `pMC`).
- `getMonsters()` returns JSON of all `world.monsters`.
- `getTargetMonster()` returns JSON of `myAvatar.target`, or `{}` (and `cancelTarget()`) if the target is missing or has HP <= 0.
- `attackMonsterName(name)` and `attackMonsterID(id)`:
  - Candidates are monsters in the current cell with a `pMC`.
  - Name matching is case-insensitive substring on `objData.strMonName`, and `"*"` matches all.
  - ID matching accepts `objData.MonMapID == id || MonID == id`.
  - Sort: alive first, then lowest HP, then lowest `MonMapID`.
  - Then `world.setTarget(t)` and `world.approachTarget()`. Returns `"true"` or `"false"`.
- Monster JSON is `objData` copied plus `intHP`, `intHPMax`, `intState` from `dataLeaf`.

**Skills and combat**
- `canUseSkill(idx)`: target exists with HP>0, the global cooldown has elapsed (`world.GCDTS + world.GCD`), the per-skill cooldown has elapsed (`skill.cd` or `OldCD` scaled by haste, where haste = `1 - clamp(myAvatar.dataLeaf.sta.$tha, -1, 0.5)`), `skill.isOK`, and `!skill.skillLock && !skill.lock`.
- `useSkill(idx)` calls `world.testAction(skill)` if the time check passes.
- `infiniteRange()` sets `world.actions.active[0..5].range = 20000`.
- `magnetize()` moves `target.pMC` to the avatar's position.
- Bug in C#: `ScriptInterface.CheckOptions` calls `Flash.Call("magnetise")`, but the registered name is `magnetize`. The `Magnetise` option property does use `magnetize`.

**Inventory**
- `hasInventoryCategories()`: whether the AS3 definition `InvCat` exists.
- `inventoryBagUsedSlots()`, `inventoryMiscSlots()`, `inventoryMiscUsedSlots()` (ints).
- `isFavoriteItem(id)` calls `liteAssets.draw.FavStore.has(id)`.
- `inventoryPool(itemJson)` calls `InvCat.poolOf`.
- `inventoryHasSpaceFor(itemJson, qty=1, purchase=false, outgoingJson=null)`:
  - Fallback without `InvCat`: true if a stack has room (`iQty < iStk`), else `items.length < iBagSlots`.
  - Otherwise `InvCat.isHouse`, `freedBy`, `isFullFor`.

**Shop**
- `buyItemByName(name, qty=-1)` and `buyItemByID(id, shopItemID, qty=-1)`:
  - Look up `world.shopinfo.items`.
  - With qty -1 they call `world.sendBuyItemRequest(item)`.
  - Otherwise they call `world.sendBuyItemRequestWithQuantity({iSel:item, iQty:qty, accept:1})`.
- `getShopItem(name)` and `getShopItemByID(itemID, shopItemID)`.
- `getUnmetPurchaseRequirements(itemID, shopItemID, qty)` returns a JSON array of strings: quest requirement (`world.getQuestValue(iQSindex) < iQSvalue`), `turnin` materials, and token (`TokID`, `TokQty`) against `world.invTree[id].iQty`.

**Auras** (lengthy; all in `Auras.as`)
- `getSubjectAuras`, `GetAuraSnapshots(subject)` (JSON array of `{nam, ...}` from `leaf.hudAuras`, with `"Self"` using `uoTree[myUserName.lower]`, otherwise the target's `dataLeaf`).
- `HasAnyActiveAura`, `GetPlayerAura`, `GetMonsterAuraByName`, `GetMonsterAuraByID`, `rebuildAuraArray`, `GetAurasValue`, `rebuilduoTree`, `rebuildmonTree`.

**Packets**
- `sendClientPacket(packet, type)` injects a fake server packet into the client, as if received:
  - `"xml"`: `new XML(packet)`.
  - `"json"`: `JSON.parse(packet)['b']`.
  - `"str"`: strips the first and last char, splits on `%`, and passes `array.splice(1)`.
  - It builds `it.gotoandplay.smartfoxserver.handlers.ExtHandler(sfc)` lazily and calls `handleMessage(obj, type)`.
- `catchPackets()`.
- To send a real client-to-server packet, C# uses `callGameFunction("sfc.sendString", "%xt%zm%...%")`.

**Utilities**
- `injectScript(uri)`: loads a SWF with `allowCodeImport` and calls `content.run(Main.instance)`.

**Remote object registry** (handle-based access)
- `lnkCreate(path)` returns an int ID as a String.
- `lnkDestroy(id)`, `lnkGetChild(id, path)`, `lnkDeleteChild`, `lnkGetValue(id)` (JSON), `lnkSetValue(id, v)`, `lnkCall(id, fn, ...args)`, `lnkGetArray(id, i)`, `lnkSetArray(id, i, v)`.
- `fcCreate(objId, name)`, `fcPush(id, argIds[])`, `fcPushDirect(id, ...args)`, `fcClear`, `fcCallFlash(id)`, `fcCall(id)`. Used by the `FlashObject<T>` wrapper in `Skua.Core/Flash`; most scripts do not use it directly.

**Modules**
- `modEnable(name)` and `modDisable(name)`: names are `QuestRequirementWiki`, `QuestItemRates`, `HidePlayers`, `DisableCollisions`, `DisableFX`.
  - Each module's `onToggle` runs on change.
  - `onFrame` runs every 100 ms while enabled.

### Events AS3 pushes to C# (`external.call` / `FlashCall`)
Handled in `Skua.Core/Scripts/ScriptInterface.cs`, `HandleFlashCall`.

| Name | Args | Meaning and C# handling |
|---|---|---|
| `requestLoadGame` | none | Sent at the end of `Externalizer.init`. `SkuaStartupHandler` responds with `Flash.Call("loadClient")`. |
| `pre-load` | none | After 1 s, C# calls `setBackgroundValues(settings.sBG ("Generic2.swf" default), CustomBackgroundPath or "")`. |
| `loaded` | none | Game SWF finished loading; starts the `ScriptInterface` 20 ms timer thread. |
| `pext` | JSON string | `JSON.stringify(sfs onExtensionResponse event)`. See below. |
| `packet` | string | Outgoing STR packet, e.g. `%xt%zm%moveToCell%...`. Only emitted for messages starting `[Sending - STR]: %xt%zm%`. |
| `packetFromServer` | string | Debug-log text of other packets, with prefixes `[Sending - STR]: `, `[ RECEIVED ]: `, `[Sending]: ` and the trailing `, (len: N)` stripped. Used by the interceptor/logger. |
| `connectionLost` | none | `sfc` lost connection; may trigger auto-relogin. |
| `userIntentionalAction` | `"logout"`, `"changeServer"`, or `"selectServer"` | Raised on clicking `btnLogout`, `btnChange`, or an `iList` item. |
| `openWebsite` | url | Opens in the browser (char page, wiki). |
| `debug` | msg | Trace output. |

**`pext` payload.**
- `packet.params.type` is `"json"` or `"str"`, and `packet.params.dataObj` is the payload.
- For `type=="json"`, `dataObj.cmd` is the command:

| cmd | Handling |
|---|---|
| `event` | `args.zoneSet` -> RunToArea message |
| `moveToArea` | map changed; sets `Map.FilePath = strMapFileName`, `LastMap = strMapName`, sends MapChangedMessage |
| `ct` | combat tick; `p[username].intHP == 0` -> death; the anim msg "prepares a counter attack!" and aura "Counter Attack" -> CounterAttackMessage |
| `sellItem`, `buyItem` | stats refresh and ItemSold / ItemBought messages |
| `dropItem` | ItemDropped message |
| `addItems` | item added to inv, temp, or bank |
| `getDrop` | pickup result |
| `addGoldExp` | `typ=="m"` -> kill count and MonsterKilled(id) |
| `ccqr` | `bSuccess==1` -> QuestTurnin |
| `loadBank` | BankLoaded |
| `loadShop` | ShopLoaded |

- For `type=="str"`, `dataObj[0]` is the command:

| cmd | Handling |
|---|---|
| `uotls` | AFK flag change |
| `loginResponse` | resets the custom name/guild, sends LoginMessage |
| `popup` | debug only |

- Every pext is also forwarded to scripts as `Events.ExtensionPacketReceived(dynamic packet)`.

**`packet` (outgoing) handler** splits on `%`, using index `[2]` as the command:
- `eLO`/`eLO2`/`eSL`/`eSL2`: intentional logout, disarms relogin and stops the script.
- `moveToCell`: CellChanged(map, parts[4], parts[5]).
- `buyItem`: TryBuyItem (parts[5], parts[4], parts[6]).
- `acceptQuest`: QuestAccepted(parts[4]).
- `cmd` with parts[4]=="logout": triggers auto-relogin unless the logout was intentional.

### Binding-generation pattern
`Skua.Core.Generators` emits C# properties from attributes. Reimplement this as TS typed getters over `getGameObject`:

| Attribute | Generated behaviour |
|---|---|
| `[ObjectBinding("path", IsStatic, RequireNotNull, Select, Default, HasSetter)]` | getter is `Flash.GetGameObject<T>(path)` (or `GetGameObjectStatic` -> `getGameObjectS`); with `Select` it calls `selectArrayObjects(path, select)`; setter calls `SetGameObject(path, value)`; `RequireNotNull` returns the default if `isNull(thatPath)` |
| `[CallBinding("fn", Get/Set/UseValue/HasSetter)]` | `Flash.Call<T>("fn")` |
| `[JsonCallBinding("fn")]` | `JsonConvert.DeserializeObject<T>(Flash.Call("fn"))` |
| `[MethodCallBinding("fn", GameFunction, RunMethodPre/Post)]` | `Flash.Call("fn", args)` or `Flash.CallGameFunction("path", args)`; the C# stub body runs before or after |
| `[ModuleBinding("Name")]` | `Flash.Call("modEnable"/"modDisable", "Name")` |

## 3. Script API (IScriptInterface and sub-interfaces)

Interface files are in `R/Skua.Core.Interfaces/Scripts/`. Implementations are in `R/Skua.Core/Scripts/`.

**Common concurrency model**
- Scripts run on a dedicated thread named `"Script Thread"`. All calls are blocking.
- `Thread.Sleep` is inside `Wait.*`. There is no async script API. Cancellation is cooperative through `Manager.ShouldExit` and the Flash-call check.

### IScriptInterface (`Bot.*`)
- Properties:
  - `Flash`, `Manager`, `Auto`, `Boosts`, `Stats`
  - `Self` (IScriptSelfAuras), `Target` (IScriptTargetAuras)
  - `AuraMonitor`, `UltraBossHelper`
  - `Combat`, `Kill`, `Hunt`, `Drops`, `Events`
  - `Reputation` (IScriptFaction)
  - `House`, `Inventory`, `TempInv`, `Bank`, `InvHelper`
  - `Lite`, `Options`, `Map`, `Monsters`, `Player`, `Quests`, `Send`, `Shops`, `Skills`, `Wait`
  - `Servers`, `Handlers`, `GameProxy`, `Accounts`
  - `Config` (IScriptOptionContainer?)
  - `ShouldExit`, `Version`, `Random`
- Methods:
  - `SellJunk()`
  - `Sleep(ms)`, `SleepAsync(ms)`
  - `Stop(bool)` (obsolete), `StopAsync(bool)`
  - `Log(string)`
  - `Schedule(delay, Action<IScriptInterface>)` and `Schedule(delay, Func<IScriptInterface, Task>)`
  - `ShowMessageBox(msg, caption, bool yesNo)`
  - `ShowMessageBox(msg, caption, params string[] buttons)`
- `Log`, `Sleep`, and `Wait.*` call `CheckScriptTermination()`. It blocks on `ScriptPauseEvent` (pause) and throws `OperationCanceledException` if `ShouldExit`.

### ScriptInterface background timer (20 ms)
Thread `"ScriptInterface"`, started on the `loaded` event:
- Computes `inGame = Flash.IsWorldLoaded && Player.LoggedIn`.
- Calls `catchPackets` once per login.
- If `Options.RestPackets` and not in combat: sends `%xt%zm%restRequest%1%%` every 1200 ms.
- Every 250 ms runs `CheckOptions`:
  - `LagKiller` -> `killLag(true)`.
  - If playing: `Magnetise`, `InfiniteRange` (`infiniteRange`), `AggroMonsters` -> `world.aggroAllMon`, `AggroAllMonsters` -> `%xt%zm%aggroMon%{roomId}%{mapIds joined by %}%`, `SkipCutscenes`, and `WalkSpeed != 8`.
  - It also forces `Lite.UntargetSelf` and `Lite.UntargetDead` to true.
- Watchdog:
  - Detects an unexpected disconnect (was playing, now not, auto relogin on, and `ReloginWatchdog` setting on) and calls `BeginLogout`.
  - Detects a kick every 1 s via `isKicked`.
  - Stuck-on-loading detection: reads `mcConnDetail.txtDetail.text` every 100 ms. "has been lost" triggers a relogin; unchanged for `Options.LoadTimeout` (30 s) triggers a relogin or logout. If the text is "loading map", it joins "battleon" and then the last map.
- Runs registered `Handlers` at `ticks*20` ms.

### Player (`IScriptPlayer`, `ScriptPlayer.cs`)
Members and the game paths they read:

| Member | Path or implementation |
|---|---|
| `ID` | `world.myAvatar.uid` |
| `XP` | `...objData.intExp` |
| `RequiredXP` | `...objData.intExpToLevel` |
| `Cell` | `world.strFrame` |
| `Pad` | `world.strPad` |
| `ServerIP` | static `serverIP` |
| `Username` | static `loginInfo.strUsername` |
| `Password` | static `loginInfo.strPassword` |
| `Guild` | `...objData.guild.Name` |
| `State` | `...dataLeaf.intState` |
| `InCombat` | State == 2 |
| `Alive` | State > 0 |
| `LoggedIn` | call `isLoggedIn` |
| `Playing` | `LoggedIn && Alive` |
| `Kicked` | false if world loaded, else call `isKicked` |
| `IsMember` | `objData.iUpgDays >= 0` |
| `Health`, `MaxHealth` | `dataLeaf.intHP`, `dataLeaf.intHPMax` |
| `Mana`, `MaxMana` | `objData.intMP`, `dataLeaf.intMPMax` |
| `Level` | `dataLeaf.intLevel` |
| `Gold`, `Coins` | `objData.intGold`, `objData.intCoins` |
| `CurrentClass` | equipped inventory item with category Class |
| `CurrentClassRank` | `objData.iRank` |
| `Target` | call `getTargetMonster` |
| `HasTarget` | target alive |
| `Loaded` | `items.length>0 && !world.mapLoadInProgress && world.myAvatar.pMC.artLoaded()` |
| `Skills` | `world.actions.active` |
| `Stats` | `dataLeaf.sta` |
| `AFK` | `dataLeaf.afk` |
| `X`, `Y` | `pMC.x`, `pMC.y` |
| `WalkSpeed` | `world.WALKSPEED`, settable |
| `Scale` | `world.SCALE`, settable |
| `AccessLevel` | `objData.intAccessLevel`, settable |

Methods:
- `Goto(name)`: `world.goto`.
- `SetSpawnPoint(cell, pad)`: `world.setSpawnPoint`.
- `WalkTo(x, y, speed=8)`: `walkTo` then `Wait.ForPlayerPosition`.
- `Rest(full=false)`: waits for the rest cooldown, calls `world.rest`, and if `full` waits until HP and MP are full.

### Map (`ScriptMap.cs`)
- Properties:
  - `Name` = `world.strMapName`.
  - `Loading` = `world.mapLoadInProgress`.
  - `Loaded` = `!Loading && isNull("mcConnDetail.stage")`.
  - `RoomID` = `world.curRoom`.
  - `PlayerCount` = `world.areaUsers.length`.
  - `PlayerNames` = `world.areaUsers`.
  - `Players` = values of `world.uoTree`.
  - `CellPlayers` = players in the same cell.
  - `Cells` = `world.map.currentScene.labels` selecting `name`.
  - `LastMap`, `FilePath`, `FileName`, and `FullName` (from `ui.mcInterface.areaList.title.t1.text`).
- `Reload()` -> `world.reloadCurrentMap`.
- `Jump(cell, pad, autoCorrect=true, clientOnly=false)`: calls `jumpCorrectRoom`, sleeps `ActionDelay`, then `Wait.ForCellChange(cell)`.
- `JoinPacket(map, cell="Enter", pad="Spawn")` sends `%xt%zm%cmd%{RoomID}%tfer%{Username}%{map}%{cell}%{pad}%`.
- `Join(map, cell="Enter", pad="Spawn", ignoreCheck=false, autoCorrect=true)`:
  1. `mapName = map.Split('-')[0]; LastMap = mapName`.
  2. Returns if the player is not playing or loaded, or if (not ignoreCheck) already in `map`.
  3. Loops `while Name != mapName && !ShouldExit && ++i < JoinMapTries`:
     - If `Options.PrivateRooms` and the map has no `-`, or `map` contains `-1e9`, then `map = mapName + "-" + (PrivateNumber>0 ? PrivateNumber : 100000)`.
     - `Wait.ForActionCooldown(Transfer)`, then `JoinPacket`.
     - If `!Wait.ForMapLoad(map, 20)`, jump to the current cell and pad; otherwise `Jump(cell, pad)`.
     - Sleep `ActionDelay`.
- `GetMapItem(id)`: `world.getMapItem(id)` after waiting for the `getMapItem` lock, plus sleep.
- `GetPlayer(username)`: cache lookup, then up to 3 tries of `world.uoTree["name"]`.
- `FindMapItems()`: downloads the map SWF, decompiles it with FFDec (`cmd /c ffdec.bat -export script`), and regex-parses `MainTimeline.as` for `getmapitem` and `isquestinprogress`. This is Windows and Java specific.

### Combat (`IScriptCombat`)
- `Attack(string name)`, `Attack(int id)`, and `Attack(Monster)`: call `attackMonsterName`/`attackMonsterID`. They return false and cancel the target if `StopAttacking` is set.
- `AttackPlayer(name)`.
- `ApproachTarget` -> `world.approachTarget`.
- `CancelTarget` -> `world.cancelTarget`.
- `CancelAutoAttack` -> `world.cancelAutoAttack`.
- `UntargetSelf` -> `untargetSelf`.
- `Exit()`: if the player is not in state 1, it cancels auto-attack and the target, jumps to the current cell and pad twice with a 300 ms sleep, then `Wait.ForCombatExit`.
- `EnableCounterHandler` and `StopAttacking`: on a CounterAttack message with `Faded=false` it stops attacking, remembers the target, and cancels. On `Faded=true` it re-attacks.

### Kill and Hunt
Kill:
- `Kill.Monster(name | id | Monster [, CancellationToken?])`:
  1. `Wait.ForMonsterSpawn(name)` (default 10 s).
  2. `Combat.Attack(name)`.
  3. Sleep `ActionDelay`.
  4. `Wait.ForMonsterDeath()`, which loops with `UntargetSelf` until `!Player.Playing || !Player.HasTarget`.
  - With a token it waits for no target or the token instead.
- `Kill.Player(name)`.
- `Kill.ForItem(name, item, qty, tempItem=false)`:
  - The `name` can be `"A|B"` (pipe-separated) or `"*"`.
  - Skips if the item is already owned (inventory, or TempInv if `tempItem`).
  - Saves the cell and pad.
  - Registers messenger handlers: `CellChanged` re-jumps back to the saved cell, and `ItemDropped` calls `Drops.Pickup(name)` then checks the quantity.
  - Loops killing monsters until the quantity is reached (the CancellationTokenSource is cancelled).
- `ForItems` loops over a list of items.

Hunt:
- `Hunt.Monster(name|id|Monster [, token])`:
  1. Finds the cells containing living monsters (`MapMonsters` from `getMonsters`, falling back to `GetLivingMonsterDataLeafCells`).
  2. For each cell other than the current one, `Map.Jump(cell, "Left")`, throttled by `HuntDelay` (1000 ms).
  3. If `Monsters.Exists(mon)` then `Combat.Attack`, sleep, and `Kill.Monster`.
- `Hunt.ForItem/ForItems` mirror the Kill versions.
- `Hunt.WithPriority(name|id|Monster, HuntPriorities, token?)`: priorities are a flags enum `None`, `HighHP`, `LowHP`, `Close`.
- `Monster` data: `MapMonsters` from `getMonsters`, `CurrentAvailableMonsters` from `availableMonsters`, `CurrentMonsters` is `MapMonsters` filtered by `Cell == Player.Cell`.
  - JSON fields: `strMonName`, `MonID`, `MonMapID`, `sRace`, `strFrame`, `intHP`, `intHPMax`, `intState`, `strMonFileName`.
  - Default interface helpers: `Exists(name or id)`, `GetLivingMonsterCells`, `GetMonstersByCell`, `TryGetMonster`.

### Quests (`ScriptQuest.cs`)
- Data: `world.questTree` (a dict), cached 100 ms in `Tree`. `Active`, `Completed` (status `"c"`), `Registered`, `RegisteredRewards`, and `Cached` (from the `QuestData.json` cache, 19 MB).
- `Load(ids)` -> `world.showQuests("1,2,3", "q")` in chunks of 30. `EnsureLoad(id)` waits (20 x 1 s) for the quest to appear in `Tree`.
- `Accept(id)`:
  1. Wait for the `acceptQuest` lock.
  2. `world.acceptQuest(id)`.
  3. `Wait.ForQuestAccept`.
  4. Returns `IsInProgress(id)` (`world.isQuestInProgress`).
- `EnsureAccept(id)` retries up to `Options.QuestAcceptAndCompleteTries` (30) times, sleeping `ActionDelay`.
- `Complete(id, itemId=-1, special=false)`:
  1. Wait for the `tryQuestComplete` lock.
  2. `world.tryQuestComplete(id, itemId, special)`.
  3. `Wait.ForQuestComplete`.
  4. Returns `!IsInProgress`.
- `EnsureComplete` retries up to 30 times.
- Checks:
  - `CanComplete(id)`: the quest is in `Completed`.
  - `CanCompleteFullCheck` also verifies requirements via `InvHelper`.
  - `IsAvailable` combines daily, unlocked, upgrade, level, class points (`getCPByID`), and faction rep (`getRep`).
  - `IsUnlocked`/`HasBeenCompleted` use `world.getQuestValue(slot)` against `value-1` and `value`.
  - `IsDailyComplete` uses `world.getAchievement(field, index) > 0`.
- `UpdateQuest(id)` injects the client packet `{"t":"xt","b":{"r":-1,"o":{"cmd":"updateQuest","iValue":V,"iIndex":S}}}` via `sendClientPacket(...,"json")`.
- Auto turn-in: `RegisterQuests(ids...)` or `(questId, rewardId)` tuples.
  - Adds the reward and requirement item IDs to `Drops.Add`, and starts `Drops.Start`.
  - Starts a "Quest Thread" polling every `RegisterCompleteInterval` (2000 ms).
  - Each pass accepts the quest if it is not in progress (2 s cooldown).
  - If `CanComplete`, it sends the raw packet `%xt%zm%tryQuestComplete%{roomId}%{questId}%{rewardId}%false%{turnIns}%wvz%` (3 s cooldown). `turnIns` is 1 if the quest is `Once` or has a `Field`, otherwise `world.maximumQuestTurnIns(id)`.
  - `UnregisterQuests` and `UnregisterAllQuests` stop it.

### Drops (`ScriptDrop.cs`)
- State: `CurrentDropInfos` is filled from `ItemDroppedMessage` (the `dropItem` pext). `ToPickup` is a set of names, `ToPickupIDs` a set of IDs, `RejectElse`, `Interval`.
- `Pickup(name or id)`: skips if the item is not in the drop list, or if it is not temp and `Inventory.HasSpaceFor` is false. It sends `%xt%zm%getDrop%{RoomID}%{itemID}%` and removes it locally.
- The array forms of `Pickup` also call `Wait.ForPickup`.
- `PickupAll()`, `PickupACItems()`.
- `RejectAll`, `RejectExcept(names or ids)`: call the `rejectExcept` callback (csv of lowercase names), then trim the local list. If `Options.AcceptACDrops` is set, AC-priced items are kept.
- `Start()` runs a PeriodicTimer loop: if `AcceptAllDrops`, `PickupAll`; if `AcceptACDrops`, pick AC items; if `RejectAllDrops`, `RejectAll`; otherwise pick listed IDs and names, then if `RejectElse`, reject everything else.
- Drops are cleared on logout. `Exists(name)` treats `"*"` as "any drop present".

### Inventory, Bank, House, TempInv
Shared `ICheckInventory<T>` default members:
- `Items`, `Contains(name|id, qty=1)`, plus a multi form `Contains(needAll, qty, params names/ids)`.
  - `Contains` is true if `Quantity >= qty` or the category is Class. `qty==0` is always true.
- `GetItem`, `TryGetItem`, `GetQuantity` (0 if missing), `IsMaxStack`.
- `Slots`, `UsedSlots`, `FreeSlots`.

Per container:
- `Inventory.Items` = `world.myAvatar.items`; `Slots` = `objData.iBagSlots`; `UsedSlots` via `inventoryBagUsedSlots`.
  - Misc pool: `MiscSlots`, `MiscUsedSlots`, `MiscFreeSlots`.
  - `GetPool`, `HasSpaceFor(item, qty)` via `inventoryHasSpaceFor`, `CanBank`, `IsFavorited`.
  - `EquipItem(id|name)`: wait `equipItem` lock, `world.sendEquipItemRequest({ItemID})`, `Wait.ForItemEquip`.
  - `EquipUsableItem`: `world.equipUseableItem`.
  - `ToBank(item)`: sends `%xt%zm%bankFromInv%{room}%{ItemID}%{CharItemID}%`, then `ForInventoryToBank`.
  - `EnsureToBank`: retries up to `MaximumTries` (10).
  - `BankAllCoinItems(...)`.
- `Bank.Items` = `world.bankinfo.items`; `Slots` = `objData.iBankSlots`; `UsedSlots` = `myAvatar.iBankCount`.
  - `Open()` -> `world.toggleBank`, skipped if `ui.mcPopup.currentLabel == "Bank"`.
  - `Load(waitForLoad)` sends `%xt%zm%loadBank%{room}%All%` and waits for a `loadBank` pext (`BankLoaded`).
  - `ToInventory(item)`: `%xt%zm%bankToInv%{room}%{ItemID}%{CharItemID}%`.
  - `Swap`: `%xt%zm%bankSwapInv%{room}%{invID}%{invCharItemID}%{bankID}%{bankCharItemID}%`.
  - `EnsureSwap`, `EnsureToInventory`.
- `House.Items` = `world.myAvatar.houseitems`; `Slots` = `objData.iHouseSlots`.
- `TempInv.Items` = `world.myAvatar.tempitems`.
- `InvHelper.Check(name|id, qty=1, moveToInventory=true)` checks TempInv, then Inventory, then Bank (moving it to inventory if requested), then House. `HasAll`/`HasAny` do the same for lists and call `Bank.Load()` first.
- `Reputation.FactionList`, `GetRank`, `HasRank`.
- Item JSON fields: `ItemID`, `CharItemID`, `sName`, `sDesc`, `iQty`, `iStk`, `iLvl`, `EnhLvl`, `EnhPatternID`, `ProcID`, `bEquip`, `bUpg`, `bWear`, `bCoins`, `bHouse`, `bTemp`, `sType`, `sES`, `sLink`, `sFile`, `sMeta`.

### Shops (`ScriptShop.cs`, `IScriptShop`)
- Members: `IsLoaded`, `ID`, `Name`, `Items`, `ProtectFavorites`, `LoadedCache`.
- `Load(id)`: `world.sendLoadShopRequest`. `LoadHairShop`, `LoadArmourCustomizer`.
- `BuyItem(name | id, shopItemId, qty)` goes through the `buyItemByName`/`buyItemByID` callbacks, with a shop-space check.
- `LoadAndBuyItem`, `EnsureLoadAndBuyItem`, `SellItem(name|id, qty)`.
- `GetUnmetPurchaseRequirements(ShopItem, qty)` uses the matching callback.

### Skills (`ScriptSkill.cs`)
- `ISkillProvider` interface (`Skua.Core.Interfaces/Skills/ISkillProvider.cs`):
  - `SkillCount`
  - `ResetOnTarget`
  - `bool? ShouldUseSkill(int skillIndex, bool canUse)`
  - `(int, int) GetNextSkill()`: returns `(sequenceIndex, skillSlot)`.
  - `bool OnTargetReset()`
  - `OnPlayerDeath()`
  - `Stop()`
  - `Load(string)` and `Save(string)`
- `Skills.Start()` starts the "Skill Timer" thread, a loop every `SkillInterval` (100 ms):
  - Waits on the pause event.
  - If `Combat.StopAttacking`, waits up to 10 s for the counter attack to fade.
  - If the player is dead, calls `provider.OnPlayerDeath`.
  - If `(AttackWithoutTarget && Playing && Loaded) || (HasTarget && Loaded && Playing)`, it polls once.
  - Always calls `provider.OnTargetReset()`.
  - The poll refreshes `Player.Skills` on a class rank change, calls `GetNextSkill`, and then `ShouldUseSkill(index, CanUseSkill(slot))`.
    - `true` uses the skill; `null` takes a fresh `GetNextSkill`; `false` skips. A skill whose `IsOk` is false is skipped.
    - `SkillUseMode.UseIfAvailable`: use only if `CanUseSkill` (the `canUseSkill` callback), otherwise skip.
    - `SkillUseMode.WaitForCooldown`: `Wait.ForTrue(CanUseSkill, SkillTimeout, SkillInterval)` then use.
    - `UseSkill(slot)` calls the `useSkill` callback.
- Members: `BaseProvider`, `OverrideProvider`, `SetProvider(p)`, `UseBaseProvider()`, `Pause()`, `Resume()`, `Stop()`, `TimerRunning`, `SkillTimeout` (-1 default), `SkillUseMode`, `SkillInterval`.
- Loaders:
  - `LoadAdvanced(skills, timeout, mode)`.
  - `LoadAdvanced(className, autoEquip, ClassUseMode.Base)`: finds saved sets by class name and mode (`Base, Atk, Def, Farm, Solo, Supp, Dodge, Ultra`). If none exist it uses the generic set (`"0 | 1 | 2 | 3 | 4"`).
  - `LoadAdvanced(className, "modeString", autoEquip)`.
  - `StartAdvanced(...)` = Load + Start.
- Skill string format (`AdvancedSkillProvider.Load`, lowercased, split on `|`): each entry is a skill digit (0-5, with 0 being auto-attack slot as used by AQW's "0") optionally followed by rules:
  - `h<50` or `h>50`: HP, `%` by default, `#` for a flat value.
  - `m<50`, `m>50#`: mana.
  - `ph>50`: party member health.
  - `a<name> ...`: aura check. Written `a>"Aura Name" 3 target` or `a<Aura 2 self`. `>` is greater, `<` less.
  - `ma...`: multi-aura. `&` is AND, `:` is OR.
  - `w`: wait rule.
  - A trailing `s` means skip if the rule is not met.
  - Rules can chain, e.g. `4 h>70 m<70 a<corporeal 0 self`.
- `AdvancedSkills.json` schema (`R/Skua.App.Avalonia/AdvancedSkills.json`):
  - `{ "<ClassName>": { "<Mode>": { "skillUseMode": "WaitForCooldown" | "UseIfAvailable", "skillTimeout": 100, "skills": [ { "skillId": 1, "rules": [ { "type": "Aura"|"Health"|"Mana"|..., "value": 0, "comparison": "less"|"greater", "isPercentage": true, "auraName": "...", "auraTarget": "self"|"target" } ] } ] } } }`.
  - The legacy text form per line is `Mode = ClassName = skills = timeout|Use if Available`.
- `AdaptiveCombat` in `MasterSkillProvider.cs` does automatic offense/defense stance switching:
  - Locks defense if HP loss is >=20% of max.
  - Locks offense if target HP drops >=40%.
  - Falls back to offense after 7 s.

### Options (`ScriptOption.cs`, `IScriptOption`)
Defaults are in parentheses, and each is persisted under the `UserOptions` setting as `Name=Value` lines.

| Option | Behaviour |
|---|---|
| `LagKiller` | `killLag(value)` (hides `world`; shows the custom BG if set) |
| `HidePlayers` | module `HidePlayers` |
| `DisableFX` | module `DisableFX` |
| `DisableCollisions` | module `DisableCollisions` (empties `world.arrSolid`/`arrSolidR`) |
| `OptimizePlayers` (true) | module `OptimizePlayers` (no AS3 implementation) |
| `SkipCutscenes` | polled by the 250 ms `CheckOptions` |
| `Magnetise` | polled |
| `InfiniteRange` | polled via `infiniteRange` |
| `AggroMonsters` | polled via `world.aggroAllMon` |
| `AggroAllMonsters` | polled via the `aggroMon` packet |
| `AttackWithoutTarget` | read by the skill loop |
| `RestPackets` | polled |
| `DisableDeathAds` | `disableDeathAd(value)` |
| `SetFPS` (30) | sets `stage.frameRate` |
| `ShowFPS` | sets `ui.mcFPS.visible` |
| `WalkSpeed` (8) | sets `world.WALKSPEED` |
| `CustomName` | writes `world.myAvatar.objData.strUsername`, `ui.mcPortrait.strName.text`, and `pMC.pname.ti.text` |
| `CustomGuild` | writes `pMC.pname.tg.text` |
| `NameColor`, `GuildColor` | `...ti.textColor`, `...tg.textColor` |
| `PrivateRooms` | used by `Map.Join` |
| `PrivateNumber` (100000) | used by `Map.Join` |
| `JoinMapTries` (3) | used by `Map.Join` |
| `ActionDelay` (800 ms) | used throughout |
| `HuntDelay` (1000) | used by Hunt |
| `HuntBuffer` (1) | used by Hunt |
| `HuntPriority` (None) | used by Hunt |
| `MaximumTries` (10) | used by Ensure* |
| `QuestAcceptAndCompleteTries` (30) | used by quest Ensure* |
| `LoadTimeout` (30000) | used by the watchdog |
| `LoginTimeout` (30000) | used by login |
| `ReloginTries` (5) | used by relogin |
| `ReloginTryDelay` (800) | used by relogin |
| `AutoRelogin`, `AutoReloginAny`, `RetryRelogin` (true), `SafeRelogin`, `ReloginServer` ("Twilly") | relogin behaviour |
| `SafeTimings` (true) | read by timing code |
| `AcceptACDrops`, `AcceptAllDrops`, `RejectAllDrops` | the latter two are mutually exclusive; changing them auto-starts `Drops` |
| `UseFunctionBasedSkills` | skill mode flag |
| `HeadlessMode` | `stage.frameRate=2`, `stage.quality="LOW"`, mixer volume 0 (restored to 30, `"HIGH"`, 1) |
| `StreamerMode` | a 500 ms loop overwrites name, guild, gold, coin, and chat visibility, and the area name text |

- Methods: `Reset()`, `ResetToDefault()`, `Save()`.
- When a script stops, `ScriptOption.ScriptStopped` resets `AutoRelogin`, `LagKiller` (toggled on then off to restore the world), `AggroAllMonsters`, `AggroMonsters`, and `SkipCutscenes` to false.

### Lite (`ScriptLite.cs`)
Every property is `ObjectBinding("litePreference.data.<key>", HasSetter=true)`. The game is the AQW "Lite" client, which has a `litePreference` SharedObject. Key mapping:

| Property | Key |
|---|---|
| `Debugger` | `bDebugger` |
| `HideUI` | `bHideUI` |
| `ShowMonsterType` | `bMonsType` |
| `SmoothBackground` | `bSmoothBG` |
| `UntargetSelf` | `bUntargetSelf` |
| `UntargetDead` | `bUntargetDead` |
| `CustomDropsUI` | `bCustomDrops` |
| `QuestLogTurnIns` | `bQuestLog` |
| `BattlePets` | `bBattlepet` |
| `StaticPlayerArt` | `bCachePlayers` |
| `ChatFilter` | `bChatFilter` |
| `ChatUI` | `bChatUI` |
| `AurasUI` | `bAuras` |
| `ColorSets` | `bColorSets` |
| `DraggableDrops` | `bDraggable` |
| `DisableChatScrolling` | `bDisChatScroll` |
| `DisableDamageNumbers` | `bDisDmgDisplay` |
| `DisableSoundFx` | `bDisSoundFX` |
| `DisableQuestPopup` | `bDisQPopup` |
| `DisableQuestTracker` | `bDisQTracker` |
| `QuestPinner` | `bQuestPin` |
| `QuestProgressNotifications` | `bQuestNotif` |
| `VisualSkillCooldowns` | `bSkillCD` |
| `HideGroundItems` | `bDisGround` |
| `HideHealingBubbles` | `bDisHealBubble` |
| `DisableAuraAnimations` | `bDisAuraAnim` |
| `HidePlayerNames` | `bHideNames` |
| `DisableDamageStrobe` | `bDisDmgStrobe` |
| `DisableMonsterAnimation` | `bDisMonAnim` |
| `DisableSelfAnimation` | `bDisSelfMAnim` |
| `DisableSkillAnimation` | `bDisSkillAnim` |
| `DisableWeaponAnimation` | `bDisWepAnim` |
| `FreezeMonsterPosition` | `bFreezeMons` |
| `InvisibleMonsters` | `bHideMons` |
| `HidePlayers` | `bHidePlayers` |
| `ReacceptQuest` | `bReaccept` |
| `CharacterSelectScreen` | `bCharSelect` |

Properties backed by `dOptions["x"]`:

| Property | Key |
|---|---|
| `DebugPacket` | `debugPacket` |
| `DisableRedWarning` | `disRed` |
| `DisableBlueMessages` | `disBlue` |
| `VanishingMessages` | `vanishMsg` |
| `Timestamps` | `timeStamps` |
| `MinimalChatMode` | `chatMinimal` |
| `DisableAutoScrollToBottom` | `chatScroll` |
| `DisableAuraTooltips` | `disAuraTips` |
| `DisableAuraText` | `disAuraText` |
| `InvertMenu` | `invertMenu` |
| `WarnWhenDecliningDrop` | `warnDecline` |
| `HideDropNotifications` | `hideDrop` |
| `HideTemporaryDropNotifications` | `hideTemp` |
| `OpenedMenu` | `openMenu` |
| `DragMode` | `dragMode` |
| `LockPosition` | `lockMode` |
| `QuantityWarnings` | `termsAgree` |
| `ShowYourSkillAnimationsOnly` | `animSelf` |
| `KeepYourWeaponAnimationsOnly` | `wepSelf` |
| `ShowNameTags` | `showNames` |
| `ShowShadows` | `showShadows` |
| `HideGuildNamesOnly` | `hideGuild` |
| `HideYourNameOnly` | `hideSelf` |
| `ShowYourGroundItemOnly` | `groundSelf` |
| `ShowYourAuraAnimationOnly` | `auraAnimSelf` |

- Generic: `Lite.Get<T>(name)` and `Lite.Set<T>(name, value)` read and write `litePreference.data.<name>`.
- The key `litePreference.data.keys['Bank']` is the bank hotkey keycode.

### Wait (`ScriptWait.cs`)
- `WAIT_SLEEP` = 100 ms. A `timeout` is a loop count: each iteration sleeps `WAIT_SLEEP` (or `sleepOverride`), so `timeout=10` is about 1 s. `timeout <= 0` means infinite. This follows the "dividing by 100" change noted in the README.
- `ForTrue(pred, [loopFn], timeout, sleepOverride=-1)` and `ForTrueAsync`.
- `For(Func<object>, value, timeout)`.
- `OverrideTimeout` plus per-category timeouts: `PlayerActionTimeout` 15, `MonsterActionTimeout` 15, `MapActionTimeout` 25, `DropActionTimeout` 10, `ItemActionTimeout` 14, `QuestActionTimeout` 14, `GameActionTimeout` 40.
- Helpers (default timeout in parentheses):
  - `ForActionCooldown(GameActions|string, 40)`: waits until `now - world.lock.<name>.ts >= world.lock.<name>.cd`. Lock names: `loadShop`, `loadEnhShop`, `loadHairShop`, `equipItem`, `unequipItem`, `buyItem`, `sellItem`, `getMapItem`, `tryQuestComplete`, `acceptQuest`, `doIA`, `rest`, `who`, `tfer`.
  - `IsActionAvailable`.
  - `ForBankLoad` (20).
  - `ForBankToInventory`, `ForInventoryToBank`, `ForBankToHouseInventory`, `ForHouseInventoryToBank` (14).
  - `ForCellChange` (WAIT_SLEEP/4).
  - `ForCombatExit` (10).
  - `ForDrop`, `ForPickup` (10).
  - `ForFullyRested` (-1).
  - `ForItemBuy`, `ForItemSell` (these are `AutoResetEvent` waits fed by buyItem/sellItem pext).
  - `ForItemEquip` (10).
  - `ForMapLoad(name, 20)`.
  - `ForMonsterDeath` (-1).
  - `ForMonsterSpawn` (10).
  - `ForPlayerPosition` (10).
  - `ForQuestAccept` (14), `ForQuestComplete` (10), `ForQuestLoad` (20).
  - `ForSkillCooldown` (50).
- Most predicates short-circuit when `!Player.Playing`.

### Send (`ScriptSend.cs`)
- `Packet(packet, type="String")` -> `callGameFunction("sfc.send" + type, packet)`. This is `sfc.sendString` for `%xt%zm%...%` packets.
- `ClientPacket(packet, type="str")` -> `sendClientPacket`. Type is `xml`, `json`, or `str`.
- `ClientServer/Event/Guild/Moderator/Warning/Whisper/Zone(msg, sentBy="Skua")` inject `%xt%chatm%0%{type}~{message}%{sentBy}%` as a client packet.
- `Whisper(name, msg)` sends `%xt%zm%whisper%1%{msg}%{name}%`.
- `PacketSpam` and `ClientPacketSpam` run background loops.

### Servers (`ScriptServers.cs`)
- Members:
  - `ServerList` = `serialCmd.servers`.
  - `CachedServers` and `GetServers(force)` from the content API.
  - `IsConnected` = `sfc.isConnected`.
  - `LastIP`, `LastName`, `IsIntentionalLogout`, `CachedUsername`.
  - `Login(user, pass)` = `game.login(user, pass)`; `Login()` uses the stored `loginInfo` values.
  - `Logout()` = `game.logout()`, then `resetConnectionState`, `sfc.disconnect`, `killLoginModals`, and `gotoAndPlay("Login")`.
  - `Connect(name|Server)`, `ConnectIP(ip[, port])` = `game.connectTo(ip[, port])`, then wait for `Playing && IsWorldLoaded` up to `LoginTimeout/100` loops.
  - `Reconnect`, `Relogin`, `EnsureRelogin`: server selection filters (online, not "Test", not full, not member-only for non-members) with fallbacks (preferred, `ReloginServer`, any).
  - `SetLoginInfo(user, pass)`.
- `EnsureLogin` loop:
  1. Refresh servers.
  2. Disconnect and back off (4 s; 10 s or more if `SafeRelogin`).
  3. `Login`.
  4. Wait for the server list (15 s cap).
  5. `loadPolicyFile`.
  6. `clickServer(name)`. If that fails, `connectToServer(serverJson)`, then `ConnectIP`.
  7. Wait for `Playing`, watching modal text via `getActiveModalMessage`.
- Auto-relogin flow: `OnLogout` stops the script, sends a ReloginTriggered message (with the kicked flag), delays (800 ms, or 10-20 s if SafeRelogin), calls `EnsureRelogin`, and restarts the script if it was running.
- `Accounts` (`IScriptAccounts`): `GetTags/HasTag/AddTag/RemoveTag/SetTags/ClearTags`, `GetAllAccounts()` returning `ManagedAccount(Username, Password, DisplayName, Tags)`.

### Events (`IScriptEvent`)
Event delegates: `Logout`, `PlayerDeath`, `MonsterKilled(mapId)`, `QuestAccepted(id)`, `QuestTurnedIn(id)`, `MapChanged(map)`, `CellChanged(map, cell, pad)`, `ReloginTriggered(wasKicked)`, `ExtensionPacketReceived(dynamic)`, `PacketReceived(string)`, `PlayerAFK`, `TryBuyItem(shopId, itemId, shopItemId)`, `CounterAttack(faded)`, `ItemDropped(item, addedToInv, qtyNow)`, `ItemSold`, `ItemBought`, `ItemAddedToBank`, `ScriptStopping(Exception?) -> bool` (the return value is the success flag), `RunToArea(zone)`. `ClearHandlers()`.

### Other sub-objects
- `Handlers.RegisterHandler(ticks, Func<IScriptInterface,bool>)` (return false removes it), `RegisterOnce`, `Remove`, `Clear`. Executed by the 20 ms timer at `ticks*20` ms intervals while a script is running.
- `Auto.StartAutoAttack/StartAutoHunt(class, ClassUseMode, manualMapIDs)`, `Stop`.
- `Boosts` (class/exp/gold/rep boost IDs, `Start/Stop`, `UseBoost`).
- `Stats`: Kills, Deaths, Drops, QuestsAccepted/Completed, Relogins, inventory and bank space, `Time`.
- `Self`/`Target` auras: `Auras`, `Snapshots`, `GetAura(name)`, `GetAuraStacks`, `HasActiveAura`, `GetAuraValue`, `HasAnyActiveAura(...)`, `GetPlayerAura`, `GetMonsterAura(name|id)`.
- `UltraBossHelper`: counter-attack and aura-analysis helpers.
- `GameProxy` (`CaptureProxy`): a local TCP MITM, see section 5.

## 4. Script compilation, loading, stop, options

### Compile (`Skua.Core/Scripts/ScriptManager.cs`, `Compiler.cs`)
- Roslyn through Westwind.Scripting `CSharpScriptExecution`, subclassed as `Skua.Core.Compiler`.
  - `OutputKind.DynamicallyLinkedLibrary`, `OptimizationLevel.Release`, `LanguageVersion.Latest`.
  - `CompileClass` finds the first exported type of the compiled assembly (the script class).
- Auto-prepended `using`s (`Skua.Core/AppStartup/Services.cs`): System, System.Collections(.Generic), System.Diagnostics, System.Drawing, System.Dynamic, System.Globalization, System.IO, System.Linq, System.Net(.Http), System.Reflection, System.Runtime.CompilerServices, System.Text(.RegularExpressions), System.Threading(.Tasks), System.Timers, System.Windows.Forms, Microsoft.Win32, Skua.Core, Skua.Core.Interfaces, Skua.Core.Models(.Items/.Monsters/.Players/.Quests/.Servers/.Shops/.Skills/.Auras/.Factions), Skua.Core.Options, Skua.Core.Utils, CommunityToolkit.Mvvm.DependencyInjection, Newtonsoft.Json(.Linq).
- References: the app's loaded assemblies plus every `*.dll` in `%APPDATA%\VibeSkua\plugins`.
- Directives (a line starting with `//cs_`, which is stripped before compile):
  - `//cs_ref <dll>` adds a reference.
  - `//cs_include <file.cs>` includes another source file.
  - Both resolve through `ScriptPathResolver` (relative to the owning file, and to the Scripts dir).
  - Includes are discovered recursively (`DiscoverAllIncludes`) and compiled as separate assemblies in a `ScriptLoadContext` (an AssemblyLoadContext that is unloaded after the run). Included files may contain nested `//cs_include`, which are only scanned at the top of the file before the first `using`.
- Version gate: a leading block comment containing `version: 1.2.3.4` is checked against the assembly version. A newer requirement throws `ScriptVersionException`.
- Cache: the SHA256 of the processed source plus include file paths and mtimes gives an int hash. Compiled DLLs go to `%APPDATA%\VibeSkua\Scripts\Cached-Scripts\{hash}-{scriptName}.dll`, with a 7-day expiry and an in-memory cap of 1024 assemblies.
- Start (`StartScript`):
  1. Stops auto and unloads the previous script.
  2. Reads and compiles the loaded file.
  3. `LoadScriptConfig(script)`.
  4. Shows the configuration UI if needed (`Config.Configure()`, skipped when `SilentConfig`).
  5. `Handlers.Clear()`.
  6. Starts the "Script Thread" (background, `Name="Script Thread"`), which creates `ScriptCts` and runs `script.GetType().GetMethod("ScriptMain").Invoke(script, [bot])`. So any public instance method `ScriptMain(IScriptInterface)` works.
  7. Sends `ScriptStartedMessage`.
- Termination: after `ScriptMain` returns or throws (`ScriptErrorMessage` for exceptions other than a user stop), the thread sends `ScriptStoppingMessage` and awaits a `ScriptStoppingRequestMessage` (2 s timeout) so that `Events.ScriptStopping` handlers run. It then calls `Skills.Stop()`, `Drops.Stop()`, `AuraMonitor.StopMonitoring()`, `UnloadPreviousScript()`, disposes the CTS, and sends `ScriptStoppedMessage`.

### Stop and cancellation
- `StopScript(runScriptStoppingEvent)`:
  1. Sets `_stoppedByScript`.
  2. Cancels `ScriptCts`.
  3. If called from the script thread, throws `OperationCanceledException` immediately.
  4. Otherwise waits up to 3 s (30 loops) for `ScriptCts == null`.
  5. Then `Thread.Join` for 5 s, then `Interrupt()` and 3 s, then 1 s more. If the thread is still stuck it logs "unresponsive".
- Cancellation points: `ShouldExit` is checked inside every `Wait.ForTrue` loop, and every `Flash.Call`, `Bot.Log`, and `Bot.Sleep` from the Script Thread throws `OperationCanceledException` when cancelled.
- Pause: `ScriptPaused` and `ScriptPauseEvent` (ManualResetEventSlim). `Wait.ForTrue`, `Log`, and `Sleep` block while paused. The UI has a Pause/Resume button.
- `RestartScriptAsync`: stop (no stopping event), wait 5 s, start.

### Script options (`ScriptOptionsInterface`)
`LoadScriptConfig` reflects public fields on the script class:
- `public string OptionsStorage = "...";` is the storage key.
- `public List<IOption> Options = new() { new Option<T>(name, displayName, description, defaultValue, transient?) ... };` uses `Skua.Core.Options.Option<T>` where `T : IConvertible` (bool, int, string, enums via string...).
- `public string[] MultiOptions = { "Sub_Group1", ... };` names more `List<IOption>` fields. Each becomes a category with `_` replaced by spaces.
- `public bool DontPreconfigure = true/false;` controls whether the options dialog is shown before the first run.
- The container is `IScriptOptionContainer`: `Storage`, `Options`, `MultipleOptions`, `OptionValues`, `SetDefaults()`, `Load()`, `Save()`, `Get<T>(IOption)`, `Get<T>(name)`, and `Configure()`.
- Scripts read values with `Bot.Config.Get<bool>("name")`.
- Persistence is a file per storage key under `%APPDATA%\VibeSkua\options`.
- `OverrideStorage` and `SilentConfig` support the unattended CLI runs.
- `Edit Script Options` re-opens the configuration. The Manager can share options across accounts via a storage override.

### Script repository
- Index: `https://raw.githubusercontent.com/auqw/Scripts/refs/heads/Skua/scripts.json` (`GetScriptsService.cs`). It is a JSON array of `ScriptInfo`:
  - `name`, `description`, `tags[]`, `path` (repo-relative), `size`, `creationDate`, `sha256`, `fileName`, `downloadUrl`.
  - The default download URL is `https://raw.githubusercontent.com/auqw/Scripts/refs/heads/Skua/{path}`.
- Commit tracking uses the GitHub API (`/repos/auqw/Scripts/commits/Skua`). The cache files are `scripts_cache.json`, `ScriptDates.json`, and `scripts-commit.txt`.
- Local dirs:
  - Settings: `%APPDATA%\VibeSkua\Skua.settings.json`.
  - Scripts: `%APPDATA%\VibeSkua\Scripts`.
  - Themes: `%APPDATA%\VibeSkua\themes`.
  - Options: `%APPDATA%\VibeSkua\options`.
  - Plugins: `%APPDATA%\VibeSkua\plugins`.
  - Skills: `%APPDATA%\VibeSkua\AdvancedSkills.json`.
  - Quests: `%APPDATA%\VibeSkua\QuestData.json`.
  - Junk: `%APPDATA%\VibeSkua\Scripts\JunkItems.json`.
- Legacy `%APPDATA%\Skua` is migrated once.

## 5. UI/UX

### Theme and colours (`Skua.Shared.Avalonia/Theming/Colors.axaml`, `BaseTheme.axaml`)
- Avalonia 11 `FluentTheme`, dark by default (`RequestedThemeVariant="Dark"`) plus Material.Icons, with light and dark dictionaries. Default font size 12.
- Dark palette:

| Role | Colour |
|---|---|
| Page | `#0A0A0A` |
| Surface | `#121212` |
| Surface alt | `#181818` |
| Border | `#282828` |
| Text | `#F5F5F5` |
| Muted text | `#A3A3A3` |
| Success | `#34D399` |
| Warning | `#FBBF24` |
| Danger | `#EF4444` (hover `#DC2626`, pressed `#B91C1C`) |

- Light palette:

| Role | Colour |
|---|---|
| Page | `#FAFAFA` |
| Surface | `#FFFFFF` |
| Surface alt | `#F5F5F5` |
| Border | `#E0E0E0` |
| Text | `#212121` |
| Muted | `#616161` |
| Success | `#059669` |
| Warning | `#B45309` |

- Accent: emerald `#10B981` (hover `#059669`, pressed `#047857`), foreground white. It is user-editable via the Application Themes window (presets, user themes, backgrounds). Resource keys use the `Skua*Brush` names.
- Custom styles: button classes `skua-button` (plus `skua-primary`, `skua-danger`, `outline`, `small`, `w84/w90/w112/w140`), `top-nav-item`, `top-toolbar` menu, `card`, `section-label`, `count`, `bottom-line` text boxes, `icon` buttons.
- Mono font stack for script names: `JetBrains Mono, Consolas, Courier New, monospace`.
- The window chrome is custom (`SystemDecorations="BorderOnly"`, `NoChrome`, with a `WindowTitleBar` control, 32 px).

### Main window (`Views/MainWindow.axaml`)
- 948x600, title "VibeSkua V{version}" (optionally with the username when `ShowUsernameInTitle` is on).
- Grid rows: custom title bar, then a 34 px top nav bar, then the game area.
- **Top nav bar** (`Background SkuaSurface`, bottom border 1 px, padding 4,0, `DockPanel`):
  - Left, the `Menu` (class `top-toolbar`), built in `AppStartup/MainMenu.cs`, in this exact order:
    1. **Scripts**: Script Loader, Scheduler.
    2. **Options**: Game, Application, CoreBots, Application Themes, HotKeys.
    3. **Tools & Helpers**: Loadouts Manager, Fast Travel, Loader, Grabber, Current Drops, Junk Items.
    4. **Combat**: Skills, Stats, Runtime.
    5. **Bank**: a direct command (opens the bank via `Bank.Open`), no submenu.
    6. **Diagnostics**: Logs, Console, Spammer, Logger, Interceptor.
    7. **Plugins** (appended last): "View Plugins" plus any plugin-registered items.
  - Right side, in order:
    - A ToggleButton "Auto" (robot icon, green 6 px dot when running) that opens a popup with the Auto panel.
    - A ToggleButton "Jump" (run-fast icon) that opens a popup with Fast Map Jump.
    - A 1 px divider.
    - A 28x28 "+" button with tooltip "Show All Bot Windows & Tools". It opens the BotWindow, a tabbed tool host.
- **Auto popup** (`AutoUserControl`): header "Combat Automation" with a "RUNNING" badge. Buttons "Auto Attack" and "Auto Hunt" (2 columns), "Stop Automation" (full width), two combo boxes ("Select Class...", "Select Mode..."), and a text box with watermark "Manual MapIDs (optional, e.g. 1, 2)".
- **Jump popup** (`JumpUserControl`): title "Fast Map Jump", combo boxes "Select Cell..." and "Select Pad...", buttons "Current" and "Jump".
- **Game area**: a `Grid` filled with `SkuaPage` colour. It holds the Flash host (hidden until `loaded`), a 150x8 indeterminate `LoadingBar` in the centre, and a headless overlay ("Headless Mode Active" / "Viewport suspended to minimize CPU and GPU usage"). There is no status bar; status is in script windows.
- **BotWindow** (the "+" tools host): a top strip with Previous/Next/First-tool buttons and the current tool title, a sidebar toggle, then a left sidebar with a "Search tools..." box and a ListBox of all managed windows, and a content area that shows the selected tool view. Every menu entry is a managed window; opening it from the menu shows it as a separate window or tab in this host.
- **TabbedHostWindow** (VibeSkua multi-client host, title "VibeSkua"):
  - Tabs per client and an "Army Control" menu (Start All Scripts, Stop All Scripts, Load Script to All, Army Scheduler, Start/Stop Auto-Attack for All, Login All Clients, Logout All Clients, Jump All to Map..., Jump All to Player..., Jump All to House, Accept Quest to All..., and Misc Options with checkable Lag Killer, Headless Mode, Hide Players, Disable FX, Infinite Range, Aggro Monsters, Streamer Mode).
  - A "Grid View" toggle.
  - Loading text "Loading Game Instance..." / "Please wait while the game client initializes".
  - Inter-client IPC uses Win32 messages (`WM_USER+449` jump map, `+450` set option, `+453` load script).

### Scripts window (Script Loader, 370x500; `Views/ScriptLoaderView.axaml`)
Rows top to bottom, margin 12, spacing 10:
1. **Header card** (rounded 10): left "LOADED SCRIPT" (9 px bold muted) over the file name (mono 13 px bold, default text "No script loaded"). Right is a pill with a status dot and text. Dot colours: muted (idle or loaded), green (running), amber (paused or compiling), red (error). Status strings: `[No Script Loaded]`, `Compiling...`, `[Running]`, `[Paused]`, `[Error]`.
2. **Buttons**, a 2-column grid with 6 px gaps:
   - Row 1: "Load Script" (folder-open icon; file dialog, 34 px high) | "Open VSCode".
   - Row 2: "Edit Script" | "Search Scripts" (opens the Script Repo window).
   - Row 3, full width: "Edit Script Options" (cog; enabled when a script is loaded).
   - Row 4: "Start Script" / "Stop Script" (toggle; primary style when idle, danger style when running) | "Pause Script" / "Resume Script" (enabled only while running).
3. **Script log**: header "SCRIPT LOG" with a count badge, then copy, save, and clear icon buttons (26 px). The log is wrapped and read-only; context menu has "Copy Selected", "Copy All", "Clear Logs", and a hint "Ctrl+C or right-click to copy selected lines".
4. **Footer** outline button: "Report Issues to @Tato2/@lonewolf_12" (opens a Discord invite).
- When the script ends the status goes back and the log keeps its text. Unattended runs use `--run-script`.

### Script Repo (`Skua.Shared.Avalonia/Controls/ScriptRepo/ScriptRepoControl.axaml`)
- Top row: a search text box ("Search scripts by name, path or tag") with a clear button, a scope combo box (width 120), and a category/filter combo box (width 140).
- Sort ascending or descending is also supported (README).
- List rows show:
  - File name (semi-bold), badges "Update" and "Local", a description, and tag chips.
  - Buttons: "Start" (primary), "Load", "Queue" (to the scheduler), and a flyout "..." with Pin/Unpin, Download (or Update), Open in VS Code, and Delete.
  - A right-click context menu with the same actions.
- Empty and loading states: "Loading scripts..." / "Getting the script list, this can take a few seconds.", and "No scripts found" / "Nothing matches the search or category." with a "Clear search" button.
- Progress overlay: a message plus a "Cancel" button.
- Bottom bar: "Update all", "Download all", and a refresh icon, plus a "..." flyout with "Update dates from Git", "Set custom scripts folder...", "Clear custom scripts folder", "Load a script file...", and "Open the Scripts folder in VS Code".

### Skills window (Combat > Skills; `Views/AdvancedSkillsView.axaml`)
- Two tabs.
- **"Create / Edit Skill"**
  - "SKILL TIMEOUT" numeric up/down.
  - "SKILLS SEQUENCE" list showing `DisplayString` per entry, with icon buttons Move Up, Move Down, Edit Selected, and Remove Selected.
  - A class name text box ("Class Name (e.g. Void Highlord)").
  - "USE MODE" check boxes: "Wait for Cooldown", "Use if available", "Reset on Target Change".
  - "USE RULES" section with check boxes "Enable Rule" and "Multi Aura", plus a "Reset" button.
    - Health, Mana, and Party rows each have a comparison toggle (>= or <=), a value, and a type toggle (% or #).
    - The multi-aura panel has an Operator combo (AND/OR), "Add Aura", and rows with a comparison, "Aura name", "Stacks", and Self/Target.
    - "Skip if not available" with a numeric box.
  - A skill slot button strip "0" to "5" (adds that skill to the sequence).
  - A class-mode combo box and a "Save Skill" button.
- **"Saved Skills"**: "Load Classes", a class combo box ("Select Class..."), a mode combo box ("Select Mode..."), and an "Open" button. A list shows each `SaveString`. Buttons: "Edit Selected", "Remove Selected", "Refresh", and an outline "Reset Sets" (restore the shipped `AdvancedSkills.json`).

### Options windows
- **Game** (Options > Game, 612x720, `GameOptionsView`):
  - A "Search game options" box.
  - Section chips: All, Visuals, Combat, Drops, Relogin, Gameplay.
  - Section titles:
    - "Performance & Visuals": LagKiller, HidePlayers, OptimizePlayers, DisableFX, DisableCollisions, ShowFPS, SetFPS (suffix "FPS"), Upgrade Name Colour, Staff Name Colour, "Reload Map" (button "Reload").
    - "Combat & Targeting": AggroMonsters, AggroAllMonsters, AttackWithoutTarget, InfiniteRange, Magnetise, HuntDelay, HuntBuffer, RestPackets, SafeTimings.
    - "Drops & Looting": AcceptACDrops (label "Accept AC Drops"), AcceptAllDrops, RejectAllDrops.
    - "Auto-Relogin & Network": the "Relogin Server" combo box (populated from the server list, falling back to Artix, Twilly, Galanoth, Gravelyn, Safiria, Sir Ver, Sepulchure, Yorumi, Alteon, Espada, Yokai, Zorbot), AutoRelogin, AutoReloginAny, RetryRelogin, SafeRelogin, ReloginTries, LoginTimeout.
    - "Gameplay & Character": everything else (PrivateRooms, PrivateNumber, SkipCutscenes, HeadlessMode, StreamerMode, CustomName, CustomGuild, WalkSpeed, ActionDelay, MaximumTries, JoinMapTries, QuestAcceptAndCompleteTries, LoadTimeout, DisableDeathAds, HuntPriority, ...), sorted alphabetically.
  - Labels are the decamelized property names ("Lag Killer"). Booleans are toggles, ints are number boxes, and strings are text boxes. Changes apply immediately.
  - Footer buttons: "Defaults" (reset to application defaults), "Reset" (reset to saved values), a note "Changes apply now; Save keeps them", and "Save".
- **Application** (`ApplicationOptionsView`) toggles and fields:
  - "Show Username in Title"
  - "Auto Update Scripts" and "Check for Script Updates"
  - "Auto Update AdvanceSkill Sets" and "Check for AdvanceSkill Sets Updates"
  - "Auto Update Junk Items" and "Check for Junk Items Updates"
  - "Discord Webhook URL" with a "Test" button
  - "Skua Nexus Webhook URL" with a "Test" button
  - Notify toggles: "Notify on Script Started/Stopped/Crashed/Relog/Item Drop", "Drop List", "Enable Health Pings", "Health Ping Interval" (min)
  - "Client Animation Frame-rate" (FPS, needs restart)
  - "Relogin Try Delay" (ms)
  - "Relogin Watchdog"
  - "Clear Flash Cache" with a "Clear" button
- Other Options windows: CoreBots (a multi-view window for CoreBots script settings: class select, loadout, class equipment, other options, via the `CBO*` views), Application Themes (Presets and User Themes lists, accent editor, backgrounds with Browse/Refresh/Get Backgrounds/Open Themes Folder), HotKeys.

### Combat > Stats, Runtime
- **Stats** (`ScriptStatsView`): the "script time" clock and a Reset button, counters KILLS, DEATHS, PICKUPS, ACCEPTED, COMPLETED, RE-LOGINS, and a STORAGE section with Inventory and Bank use and a "Get space" button.
- **Runtime** (`RuntimeHelpersView`): sections "DROPS HELPER", "QUESTS HELPER", "BOOSTS HELPER".
  - DROPS HELPER: a list with "Remove Selected", a "Drop Name (e.g. Unidentified 13)" box with "Add Drop", an "Interval (ms)" box, check boxes "Reject Else", "Accept AC Drops", "Pick Drops", "Notifications".
  - QUESTS HELPER: "Quest ID", "Reward ID (opt)", "Add Quest".
  - BOOSTS HELPER: "Search Inventory", "Search Bank", "Class Boost", "Gold Boost", "Experience Boost", "Reputation Boost", "Use Boosts".

### Tools and Diagnostics windows
- **Loader**: tabs or radio "Shop" and "Quests". An ID box ("IDs, e.g. 2566 or 101, 102") with "Load". "QUEST CACHE" with a search box ("Search by name"), "Update cache", "Add missing", "Rebuild all", "Cancel", a range box ("Range, e.g. 2566, 2599") with "Update range". Selection tools: "Copy" (menu: "Copy IDs", "Copy names", "Copy as // Name | ID"), "Fake complete" (sends a quest-complete packet to the client), "Load selected".
- **Grabber**: reads game data into lists (shops, quests, inventory, bank, monsters, and so on) for scripting.
- **Fast Travel**: search box ("Search name, map or cell"), a "Private room" toggle with a "Room" number, a "DESTINATIONS" list (row actions Travel, Edit, Remove), "Clear" and "Save", and an add form with Name / Map / Cell / Pad, "Get current", "Add".
- **Current Drops**: search box ("Search drops"), AC badges, "AC only", "Pick up all", "Pick up selected", "None selected".
- **Junk Items**: manages the sell-all list (`JunkItems.json`).
- **Diagnostics > Logs**: tabbed log views. Each has Copy, Save, and Clear buttons.
- **Diagnostics > Console**: a C# snippet runner. Header "C# SNIPPET", the hint "runs inside ScriptMain(IScriptInterface Bot)", then "Clear" and "Run".
- **Spammer**: "Start"/"Stop", "Every [ms] ms", a "Send to client" check box (uses `sendClientPacket` instead of the server), a "QUEUE" list with "Load", "Save", "Clear", "Add", and "Send now".
- **Logger**: "Logging" start/stop toggle, a "Search packets" box, a filter button (shows `n / total` filters; has select all, clear, and invert), a list capped at 1000 lines (outgoing packets only: the `packet` event, e.g. `%xt%zm%...%`), and Copy, Save, Clear.
- **Interceptor**: a server combo box ("Select server...") with a refresh button, "Connect" and "Disconnect" buttons, a "Log packets" toggle, a "Connected" indicator, a search box, and Copy, "Export" (lines marked OUT, IN, or BLOCK), Clear. It uses `CaptureProxy`: a local `TcpListener` on `127.0.0.1:5588` forwarding to the real game server. Packets are NUL-terminated, 4096-byte read buffer; inbound ones may be compressed (`_TryUnpack`/`_Pack`). `IInterceptor.Intercept(MessageInfo, bool outbound)` can edit `Content` or set `Send=false` to drop.
- **HotKeys**: a binding editor over a registry of commands, such as `OpenBank`, `ToggleHeadlessMode`, `ToggleStreamerMode`, `ToggleHidePlayers`, `ToggleDisableFX`, `ToggleMagnetise`, `ToggleInfiniteRange`, `Logout`, `StopScript`, `JumpToHome`, `ToggleRejectAllDrops`, `ToggleAggroAllMonsters`, and `Army*` broadcasts.

### Manager app (`Skua.Manager.Avalonia`)
- 850x500 custom window. Title "VibeSkua Manager V{version}".
- A top tab strip (class `top-tabs-client`), in this order: **Accounts, Army Queue, Launcher, Updater, Scripts, Options, Themes, About**.
- **Accounts tab** (`AccountManagerUserControl`):
  - Headers "Accounts" and "Groups". A Tags column and filter popup ("Filter by Tags", "Search tags...", "Clear All"). Toggles for grid or list view, grid columns, and tag visibility.
  - Add-account form: "Username", "Password" (with a show/hide button), "Display Name (optional)", add and cancel.
  - Toolbar: "Add Account", "Select All", "Unselect Selected", "Remove Selected", Move Up and Move Down, Refresh (F5).
  - Launch controls: a server combo box ("Which server to login when launching accounts"), "Start Selected", "Start All", plus "in a separate window" variants.
  - Script row: a "Browse and download scripts" button, a script file picker, a path box, an "Auto-start" check box, "Edit Options", and "Save Setup" (saves selected accounts, script path, and auto-start).
  - Per-account right-click menu: Start, Start With Script, Select / Check, Add to Group, Edit Tags, Edit Account, Copy Username, Remove.

## 6. Login flow and Account Manager

### Login in the client
1. The client opens, `skua.swf` fires `requestLoadGame`, C# calls `loadClient`, the game loads from game.aq.com, and `loaded` fires. The game's own login screen (`mcLogin`) is shown.
2. A human normally types the username and password into the stock AQW login form and picks a server from the stock server list. The game itself handles credentials via `game.params.loginURL` (`api/login/now`).
3. Automated login happens only via CLI args (`SkuaStartupHandler`, `StartupOptions`):

| Flag | Meaning |
|---|---|
| `-u/--user` | username |
| `-p/--password` | password |
| `-s/--server` | default "Twilly" |
| `--run-script <path>` | load and start the script |
| `--load-script <path>` | load only |
| `--script-options-storage <key>` | override the option storage key |
| `--use-theme <name>` | theme |
| `--gh-token <token>` | GitHub token |
| `--exit-on-script-stop` / `--exit-on-finish` | exit the process after the script stops |
| `--embed`, `--host-pid <n>` | host-window embedding |

4. With `-u` and `-p`: `Servers.SetLoginInfo(user, pass)`, and on `loaded` it calls `Servers.EnsureRelogin(server)`. That performs `Login(user, pass)` (which calls `game.login(user, pass)`, i.e. the stock login function), waits for the server list to be ready, calls `clickServer(name)` (falling back to `connectToServer` or `ConnectIP`), and waits for the world. It waits up to 80 x 250 ms for `Player.Playing`. Then, after 1 s, it starts or loads the requested script.
5. Server list source: `serialCmd.servers` (from the game) or the `content.aq.com` API.
6. `Security.loadPolicyFile("xmlsocket://ip:port")` is required for each server before connecting.
7. Auto-relogin is described in the Servers section above.

### Account Manager (Manager app)
- Accounts are stored in `Skua.settings.json` under `Manager.ManagedAccounts`. The shape is a `Dictionary<username, AccountData>`:
  - `DisplayName`
  - `Password` (stored in the clear in the settings JSON)
  - `Tags[]`
  - `UseCheck` (the "selected" flag)
- Groups are `Manager.Groups` (name plus a list of account names).
- Launching an account starts the client executable (`SkuaExecutableLocator.ExecutablePath`) with the flags `-u user -p pass -s server`, plus `--host-instance`, `--use-theme`, and `--run-script`/`--load-script`. Staggered batch launches use a 500-900 ms delay with jitter.
- Other Manager tabs:
  - **Army Queue**: Squads of accounts with per-squad default scripts and per-player role scripts, Start / Pause / Resume / Stop Queue, presets.
  - **Launcher**: a process list for running clients.
  - **Updater**: client self-update using Velopack, via GitHub Releases.
  - **Scripts**: script update (`Update Scripts`, `Reset Script Cache`).
  - **Options**: "Use Manager theme on VibeSkua".
  - **Themes**, **About**.

## 7. Notes for the Rust/Ruffle port
1. The two-way surface is small: about 70 ExternalInterface callbacks (section 2) plus 9 outbound events. Most C# API behaviour is generic dotted-path reads of AQW game state (`world.*`, `sfc.*`, `litePreference.*`, `ui.*`). The port needs a way to evaluate those paths inside Ruffle's AVM2 and to run a small amount of AS logic (jump correction, monster targeting, skill timing check, shop and inventory space).
2. The AS3 host (`Main.as` and `api/*.as`) is about 3,000 lines. It can be reused as is only if Ruffle supports the APIs it uses (see section 0). Otherwise, re-implement those functions in Rust against Ruffle's object model.
3. Timing semantics scripts depend on: `Wait` timeouts count loops of 100 ms; the `ActionDelay` default is 800 ms; global action locks are read from `world.lock.<action>.ts/.cd`; quest and drop auto-handlers poll at 2 s and 1 s cadences.
4. The packet string format is SmartFox `%xt%zm%{cmd}%{roomId}%args%` for sends. Incoming packets arrive via the `onExtensionResponse` event as `{params:{type, dataObj}}`.
5. The game client is the official AQW Lite build from `https://game.aq.com/game/gamefiles/` (the file name comes from `api/data/gameversion` -> `sFile`); the C# code depends on that build's internals (`litePreference`, `InvCat`, `cDropsUI`, `liteAssets.draw.FavStore`, `ModalMC`, ...).
6. Known quirks to avoid copying:
   - `magnetise` vs `magnetize` call-name mismatch.
   - `ModuleBinding("OptimizePlayers")` with no module.
   - `ScriptOption.ScriptStopped` toggling `LagKiller` true then false.
   - `ForCellChange` uses `WAIT_SLEEP / 4` as its timeout.
   - Account passwords are stored as plain text.
