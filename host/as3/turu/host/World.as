package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.events.TimerEvent;
import flash.system.ApplicationDomain;
import flash.text.TextField;
import flash.utils.Timer;
import flash.utils.getQualifiedClassName;

/**
 * You and the world (docs/VIBESKUA_SPEC.md, "Player", "World" and "Packets"): logged in or
 * kicked, gender, other avatars, walking, moving between cells, the open dialog, packets
 * handed to the client as if the server sent them, and the registered quests' state.
 */
public class World {
    /** `jumpCorrectRoom`'s check: every 50 ms, 40 times. */
    private static const CHECK_MS:int = 50;
    private static const CHECKS:int = 40;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    /** SmartFox's extension handler, made once (`sendClientPacket`). */
    private var extHandler:*;
    private var extHandlerOf:*;
    private var checking:Timer;

    public function World(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`; reads are quiet. */
    public function exposeWith(expose:Function):void {
        expose("isLoggedIn", isLoggedIn, undefined, true);
        expose("isKicked", isKicked, undefined, true);
        expose("gender", gender, undefined, true);
        expose("getAvatar", getAvatar, undefined, true);
        expose("getActiveModalMessage", getActiveModalMessage, undefined, true);
        expose("questStates", questStates, undefined, true);
        expose("walkTo", walkTo);
        expose("jumpCorrectRoom", jumpCorrectRoom);
        expose("sendClientPacket", sendClientPacket);
    }

    private function get world():* {
        return gameOf().world;
    }

    private function isLoggedIn():String {
        var game:* = gameOf();
        return String(game != null && game.sfc != null && game.sfc.isConnected == true);
    }

    /** The login screen's warning is up (the server said you were kicked); false in game. */
    private function isKicked():String {
        try {
            return String(gameOf().mcLogin.warning.visible == true);
        } catch (e:Error) {
        }
        return "false";
    }

    /** The client's own `strGender` (`"M"` or `"F"`), as JSON. */
    private function gender():String {
        return JSON.stringify(world.myAvatar.objData.strGender);
    }

    /**
     * The registered-quest loop's read, once a pass rather than twice a quest: for each quest ID,
     * whether the game has it in progress (`world.isQuestInProgress`) and its `world.questTree`
     * entry (null when it isn't loaded), as `{"<id>": {"inProgress": bool, "quest": entry}}`.
     */
    private function questStates(ids:*):String {
        var w:* = world;
        var out:Object = {};
        for each (var id:* in ids) {
            var quest:* = w.questTree != null ? w.questTree[String(id)] : null;
            out[String(id)] = {
                inProgress: w.isQuestInProgress(int(id)) == true,
                quest: quest === undefined ? null : quest
            };
        }
        return JSON.stringify(out);
    }

    private function getAvatar(id:*):String {
        return JSON.stringify(world.avatars[id].objData);
    }

    /**
     * The text of the dialog on screen, first found of: the options menu's, the login screen's
     * and the game's modal stacks, then the login warning; `""` when none is open. Plain text,
     * as VibeSkua's.
     */
    private function getActiveModalMessage():String {
        var game:* = gameOf();
        var stacks:Array = [];
        for each (var reach:Function in [
            function():* { return game.mcO.ModalStack; },
            function():* { return game.mcLogin.ModalStack; },
            function():* { return game.ui.ModalStack; }
        ]) {
            try {
                stacks.push(reach());
            } catch (e:Error) {
            }
        }
        for each (var stack:* in stacks) {
            if (stack is DisplayObjectContainer && stack.numChildren > 0) {
                var text:String = textIn(stack.getChildAt(0));
                if (text != null) {
                    return text;
                }
            }
        }
        try {
            var warning:* = game.mcLogin.warning;
            if (warning != null && warning.visible) {
                var said:String = textIn(warning);
                if (said != null) {
                    return said;
                }
            }
        } catch (e:Error) {
        }
        return "";
    }

    /** The text fields' text under `at`, joined by spaces; null without any. */
    private static function textIn(at:DisplayObject):String {
        var texts:Array = [];
        collectText(at, texts);
        return texts.length > 0 ? texts.join(" ") : null;
    }

    private static function collectText(at:DisplayObject, into:Array):void {
        if (at is TextField) {
            var text:String = TextField(at).text;
            if (text != null && text.replace(/\s+/g, "") != "") {
                into.push(text);
            }
            return;
        }
        var container:DisplayObjectContainer = at as DisplayObjectContainer;
        if (container == null) {
            return;
        }
        for (var i:int = 0; i < container.numChildren; i++) {
            collectText(container.getChildAt(i), into);
        }
    }

    /** Walks you to (x, y); speed 8 is the game's own walking speed. */
    private function walkTo(x:*, y:*, speed:* = 8):void {
        var sp:Number = Number(speed) == 8 ? Number(world.WALKSPEED) : Number(speed);
        var me:* = world.myAvatar.pMC;
        me.walkTo(int(x), int(y), sp);
        world.moveRequest({mc: me, tx: int(x), ty: int(y), sp: sp});
    }

    /**
     * Moves you to `cell` at `pad`. With `autoCorrect`, once the map has settled on the cell,
     * moves again, as VibeSkua's does: to `pad` if the cell has it, else "Left", else its first.
     */
    private function jumpCorrectRoom(cell:*, pad:*, autoCorrect:* = true, clientOnly:* = false):void {
        var toCell:String = String(cell);
        var toPad:String = String(pad);
        var onlyClient:Boolean = clientOnly === true || clientOnly == "true";
        world.moveToCell(toCell, toPad, onlyClient);
        if (checking != null) {
            checking.stop();
            checking = null;
        }
        if (!(autoCorrect === true || autoCorrect == "true")) {
            return;
        }
        checking = new Timer(CHECK_MS, CHECKS);
        checking.addEventListener(TimerEvent.TIMER, function(event:TimerEvent):void {
            var map:* = world.map;
            if (map == null || map.currentLabel != toCell || map.isPlaying) {
                return;
            }
            Timer(event.target).stop();
            checking = null;
            // A pad is a map child whose class is a pad's (as the client's cellSetup finds
            // them); its name is the pad's.
            var pads:Array = [];
            for (var i:int = 0; i < map.numChildren; i++) {
                var child:DisplayObject = map.getChildAt(i);
                if (getQualifiedClassName(child).indexOf("Pad") >= 0) {
                    pads.push(child.name);
                }
            }
            var chosen:String = toPad;
            if (pads.length > 0 && pads.indexOf(toPad) < 0) {
                chosen = pads.indexOf("Left") >= 0 ? "Left" : pads[0];
            }
            world.moveToCell(toCell, chosen, onlyClient);
        });
        checking.start();
    }

    /**
     * Hands `packet` to the client as if the server sent it: `xml`, `json` (its `b`) or `str`
     * (`%xt%cmd%...%`: its fields after the first).
     */
    private function sendClientPacket(packet:*, type:*):void {
        var text:String = String(packet);
        var kind:String = String(type);
        var message:*;
        if (kind == "xml") {
            message = new XML(text);
        } else if (kind == "json") {
            message = JSON.parse(text)["b"];
        } else {
            message = text.substr(1, text.length - 2).split("%").splice(1);
        }
        handlerFor(gameOf().sfc).handleMessage(message, kind);
    }

    private function handlerFor(sfc:*):* {
        if (extHandler == null || extHandlerOf !== sfc) {
            var domain:ApplicationDomain = gameOf().loaderInfo.applicationDomain;
            var Handler:Class = Class(domain.getDefinition("it.gotoandplay.smartfoxserver.handlers.ExtHandler"));
            extHandler = new Handler(sfc);
            extHandlerOf = sfc;
        }
        return extHandler;
    }
}
}
