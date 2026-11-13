package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.display.Loader;
import flash.display.MovieClip;
import flash.display.Stage;
import flash.events.ErrorEvent;
import flash.events.Event;
import flash.events.IOErrorEvent;
import flash.events.KeyboardEvent;
import flash.events.MouseEvent;
import flash.events.SecurityErrorEvent;
import flash.events.UncaughtErrorEvent;
import flash.net.URLLoader;
import flash.net.URLRequest;
import flash.system.ApplicationDomain;
import flash.system.Security;
import flash.text.TextField;
import flash.text.TextFieldType;
import flash.utils.setTimeout;

/**
 * Turu's host SWF (docs/HOST_SWF_PROPOSAL.md): loads AQW's client and answers the bot's calls,
 * with VibeSkua's names and answers (docs/VIBESKUA_SPEC.md, sections 1 and 2).
 *
 * Here: the start-up (the client loaded with its `params`, the stage, the login background, the
 * clicks the bot must know of: logout, change server, a server picked; the Bank hotkey), packets
 * and errors. The calls are in [GameCalls], [Combat], [Auras], [Items], [World], [Servers] and
 * [Options]; what changed in game reaches Turu through [Events]; the host (Ruffle's or Turu Mas's) is behind [Host].
 */
public class Main extends MovieClip {
    /** `hostInfo().version`: raised when a callback changes what it answers. */
    private static const VERSION:int = 6;
    private static const GAME_URL:String = "https://game.aq.com/game/";
    /** The login screen's title (VibeSkua's says "AURAS!!!"). */
    private static const TITLE:String = '<font color="#FDAF2D">Turu</font>';
    private static const VERSION_TRIES:int = 3;

    private var host:Host;
    private var stageRef:Stage;
    private var packets:Packets;
    private var options:Options;
    private var background:Background;
    /** Kept: nothing else holds it, and its frame listener would go with it. */
    private var events:Events;
    /** The callbacks `hostInfo` lists. */
    private var callbacks:Array = [];

    private var loading:Boolean = false;
    private var versionTries:int = 0;
    /** `api/data/gameversion`'s answer (`sFile`, ...), the client's `params.vars`. */
    private var vars:Object;
    private var loader:Loader;
    /** The client's root (its `Game`), once loaded. */
    private var game:*;
    private var paramsSet:Boolean = false;

    public function Main() {
        super();
        if (stage != null) {
            start();
        } else {
            addEventListener(Event.ADDED_TO_STAGE, onAddedToStage);
        }
    }

    private function onAddedToStage(event:Event):void {
        removeEventListener(Event.ADDED_TO_STAGE, onAddedToStage);
        start();
    }

    private function start():void {
        // Kept: this SWF leaves the stage when the game takes its place.
        stageRef = stage;
        try {
            Security.allowDomain("*");
        } catch (e:Error) {
            // AIR's application sandbox throws.
        }
        var ours:Object = loaderInfo.parameters;
        patchTrim();
        // The client's errors too: they pass up through its Loader to this SWF's.
        loaderInfo.uncaughtErrorEvents.addEventListener(UncaughtErrorEvent.UNCAUGHT_ERROR, onUncaughtError);
        // Turu Mas: its host compiles in `TuruHost`, and names this SWF's tab.
        if (ApplicationDomain.currentDomain.hasDefinition("TuruHost")) {
            host = new AirHost(int(ours.turuClient));
        } else {
            host = new ExternalHost(stageRef);
        }
        packets = new Packets(host);
        background = new Background(host, gameOf);
        events = new Events(host, gameOf);
        events.exposeWith(expose);

        expose("loadClient", loadClient);
        expose("setBackgroundValues", background.setValues);
        expose("hostInfo", hostInfo);
        expose("setPacketCapture", packets.setCapture);
        expose("catchPackets", catchPackets);
        new GameCalls(gameOf).exposeWith(expose);
        new Combat(gameOf).exposeWith(expose);
        new Auras(gameOf).exposeWith(expose);
        new Items(gameOf).exposeWith(expose);
        new World(gameOf).exposeWith(expose);
        new Servers(gameOf).exposeWith(expose);
        options = new Options(gameOf);
        options.exposeWith(expose);
        var animations:Animations = new Animations(gameOf);
        new Levers(gameOf, events, animations).exposeWith(expose);
        // For checking error reports by hand; not listed, so only debug builds of Turu call it.
        host.addCallback("throwForTest", guard("throwForTest", throwForTest));

        stageRef.addEventListener(KeyboardEvent.KEY_DOWN, onKeyDown);
        host.call("requestLoadGame", []);
    }

    /** VibeSkua's `String.prototype.trim`, which the game's code calls. */
    private static function patchTrim():void {
        var proto:Object = String.prototype;
        proto.trim = function():String {
            return String(this).replace(/^\s+|\s+$/g, "");
        };
        proto.setPropertyIsEnumerable("trim", false);
    }

    private function gameOf():* {
        return game;
    }

    // ---- Callbacks ------------------------------------------------------------------------

    /**
     * Registers a callback, guarded, and lists it in `hostInfo`. `onThrow` is its answer when it
     * throws (VibeSkua's: nothing for most, `""` where its callback catches). A `quiet` one's
     * throws aren't reported: a path read before it resolves (`world.myAvatar` before login) is
     * an ordinary empty answer.
     */
    private function expose(name:String, fn:Function, onThrow:* = undefined, quiet:Boolean = false):void {
        callbacks.push(name);
        host.addCallback(name, guard(name, fn, onThrow, quiet));
    }

    /**
     * `fn` in a `try`: a throw is reported (`error`) and answers `onThrow` (nothing for most).
     */
    private function guard(name:String, fn:Function, onThrow:* = undefined, quiet:Boolean = false):Function {
        return function(... args):* {
            try {
                return fn.apply(null, args);
            } catch (e:*) {
                if (!quiet) {
                    report(name + ": " + messageOf(e), stackOf(e));
                }
            }
            return onThrow;
        };
    }

    private function hostInfo():String {
        return JSON.stringify({
            name: "turu",
            version: VERSION,
            features: ["errors", "packetCapture", "loadClientFrameRate", "events"],
            callbacks: callbacks
        });
    }

    /** VibeSkua's, called once per login: the SmartFox client is followed each frame anyway. */
    private function catchPackets():void {
        if (game != null) {
            packets.frame(game);
        }
    }

    /** Throws now, or with "later" outside any callback (the uncaught-error path). */
    private function throwForTest(when:* = undefined):void {
        if (when == "later") {
            setTimeout(function():void {
                throw new Error("throwForTest, uncaught");
            }, 0);
            return;
        }
        throw new Error("throwForTest");
    }

    // ---- Loading the client ---------------------------------------------------------------

    /**
     * Fetches the game's version, then loads its client, first setting the stage to `frameRate`
     * (Turu passes the client's animation frame rate).
     */
    private function loadClient(frameRate:* = undefined):void {
        var fps:Number = Number(frameRate);
        if (fps > 0 && host.ownsStage()) {
            stageRef.frameRate = fps;
        }
        if (loading) {
            return;
        }
        loading = true;
        fetchVersion();
        host.call("pre-load", []);
    }

    private function fetchVersion():void {
        versionTries++;
        var fetch:URLLoader = new URLLoader();
        fetch.addEventListener(Event.COMPLETE, onVersion);
        fetch.addEventListener(IOErrorEvent.IO_ERROR, onVersionFailed);
        fetch.addEventListener(SecurityErrorEvent.SECURITY_ERROR, onVersionFailed);
        fetch.load(new URLRequest(GAME_URL + "api/data/gameversion"));
    }

    private function onVersionFailed(event:ErrorEvent):void {
        if (versionTries < VERSION_TRIES) {
            host.call("debug", ["Couldn't get the game's version, trying again: " + event.text]);
            setTimeout(fetchVersion, 2000);
        } else {
            report("Couldn't get the game's version: " + event.text, "");
            loading = false;
        }
    }

    private function onVersion(event:Event):void {
        try {
            vars = JSON.parse(String(URLLoader(event.target).data));
        } catch (e:Error) {
            report("The game's version isn't JSON: " + e.message, stackOf(e));
            loading = false;
            return;
        }
        loader = host.load(GAME_URL + "gamefiles/" + vars.sFile + "?ver=" + Math.random());
        loader.contentLoaderInfo.addEventListener(Event.INIT, onClientInit);
        loader.contentLoaderInfo.addEventListener(Event.COMPLETE, onClientLoaded);
        loader.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, onClientFailed);
    }

    private function onClientFailed(event:IOErrorEvent):void {
        report("Couldn't load the game: " + event.text, "");
        loading = false;
    }

    /** Before the client reaches the stage, which is when it reads its `params`. */
    private function onClientInit(event:Event):void {
        game = loader.content;
        setParams();
    }

    private function setParams():void {
        var params:Object = game.params;
        if (params == null) {
            return;
        }
        var ours:Object = loaderInfo.parameters;
        for (var key:String in ours) {
            params[key] = ours[key];
        }
        params.vars = vars;
        params.sURL = GAME_URL;
        params.sBG = background.sBG;
        params.sTitle = TITLE;
        params.isEU = ours.isEU === true || ours.isEU == "true";
        params.loginURL = GAME_URL + "api/login/now";
        paramsSet = true;
    }

    private function onClientLoaded(event:Event):void {
        game = loader.content;
        if (!paramsSet) {
            setParams();
        }
        host.show(this, DisplayObject(game));
        game.addEventListener(Event.ENTER_FRAME, onFrame);
        // Capture: before the game's own handlers, which may stop the click.
        game.addEventListener(MouseEvent.CLICK, onClick, true);
        packets.frame(game);
        host.call("loaded", []);
    }

    // ---- In game --------------------------------------------------------------------------

    private function onFrame(event:Event):void {
        packets.frame(game);
        try {
            options.frame();
        } catch (e:Error) {
            report("modules: " + messageOf(e), stackOf(e));
        }
    }

    /**
     * The clicks the bot must know of (`userIntentionalAction`): Logout and Change Server in
     * the game's options menu (`mcOption`), and a server picked in the login screen's list.
     */
    private function onClick(event:MouseEvent):void {
        var at:DisplayObject = event.target as DisplayObject;
        while (at != null && at != game) {
            var parent:DisplayObjectContainer = at.parent;
            if (at.name == "btnLogout") {
                host.call("userIntentionalAction", ["logout"]);
                return;
            }
            if (at.name == "btnChange") {
                host.call("userIntentionalAction", ["changeServer"]);
                return;
            }
            if (parent != null && parent.name == "iList" && inServerList(parent)) {
                host.call("userIntentionalAction", ["selectServer"]);
                return;
            }
            at = parent;
        }
    }

    /** Whether `list` is the login screen's server list, `mcLogin.sl.iList`. */
    private function inServerList(list:DisplayObjectContainer):Boolean {
        try {
            return list.parent == game.mcLogin.sl;
        } catch (e:Error) {
        }
        return false;
    }

    /** The game's Bank key (`litePreference.data.keys.Bank`) opens the bank, unless typing. */
    private function onKeyDown(event:KeyboardEvent):void {
        if (game == null || !host.isShown()) {
            return;
        }
        var focus:TextField = stageRef.focus as TextField;
        if (focus != null && focus.type == TextFieldType.INPUT) {
            return;
        }
        try {
            var key:* = game.litePreference.data.keys["Bank"];
            if (key != null && event.keyCode == key && game.world != null) {
                game.world.toggleBank();
            }
        } catch (e:Error) {
            // Not in game yet.
        }
    }

    // ---- Errors ---------------------------------------------------------------------------

    /** This SWF's and the client's uncaught errors, to the bot's debug log. */
    private function onUncaughtError(event:UncaughtErrorEvent):void {
        report(messageOf(event.error), stackOf(event.error));
    }

    private function report(message:String, stack:String):void {
        host.call("error", [message, stack]);
    }

    // `as`, not `Error(e)`: calling `Error` makes a new error rather than casting.
    private static function messageOf(e:*):String {
        var error:Error = e as Error;
        if (error != null) {
            var id:String = error.errorID != 0 ? " #" + error.errorID : "";
            return error.name + id + ": " + error.message;
        }
        var event:ErrorEvent = e as ErrorEvent;
        if (event != null) {
            return event.type + ": " + event.text;
        }
        return String(e);
    }

    private static function stackOf(e:*):String {
        var error:Error = e as Error;
        var stack:String = error != null ? error.getStackTrace() : null;
        return stack != null ? stack : "";
    }
}
}
