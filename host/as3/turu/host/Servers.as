package turu.host {
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.events.MouseEvent;
import flash.events.TimerEvent;
import flash.system.ApplicationDomain;
import flash.system.Security;
import flash.utils.Timer;
import flash.utils.getQualifiedClassName;

/**
 * The server list and logging in (docs/VIBESKUA_SPEC.md, "Server and login"). `connectToServer`
 * makes the stock server list's own checks (the client's `ServerList.onServerClick`, with its
 * messages) before connecting.
 */
public class Servers {
    /** `connectToServer` waits this long (100 ms x 50) for the login answer (`Game.objLogin`). */
    private static const WAIT_MS:int = 100;
    private static const WAITS:int = 50;

    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    /** `connectToServer` waiting for the login answer. */
    private var waiting:Timer;

    public function Servers(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`. */
    public function exposeWith(expose:Function):void {
        expose("clickServer", clickServer);
        expose("connectToServer", connectToServer);
        expose("killLoginModals", killLoginModals);
        expose("resetConnectionState", resetConnectionState);
        expose("loadPolicyFile", loadPolicyFile);
        expose("loadPolicyFiles", loadPolicyFiles);
    }

    private function gameClass():* {
        var game:* = gameOf();
        var domain:ApplicationDomain = game.loaderInfo.applicationDomain;
        return domain.getDefinition(getQualifiedClassName(game));
    }

    /** The login screen's server list entries (`mcLogin.sl.iList`'s children). */
    private function serverEntries():Array {
        var entries:Array = [];
        try {
            var list:DisplayObjectContainer = gameOf().mcLogin.sl.iList;
            for (var i:int = 0; i < list.numChildren; i++) {
                entries.push(list.getChildAt(i));
            }
        } catch (e:Error) {
        }
        return entries;
    }

    /** Clicks the first server in the list whose name contains `name`; whether one was found. */
    private function clickServer(name:*):String {
        killLoginModals();
        loadPolicyFiles();
        var wanted:String = String(name).toLowerCase();
        for each (var entry:* in serverEntries()) {
            var shown:String = "";
            try {
                shown = String(entry.tName.ti.text).toLowerCase();
            } catch (e:Error) {
                continue;
            }
            if (shown.indexOf(wanted) >= 0) {
                DisplayObject(entry).dispatchEvent(new MouseEvent(MouseEvent.CLICK));
                return "true";
            }
        }
        return "false";
    }

    /** Connects to the server `json` describes, once logged in, as the stock list would. */
    private function connectToServer(json:*):void {
        var server:Object = JSON.parse(String(json));
        resetConnectionState();
        loadPolicyFile(server.sIP, server.iPort);
        waiting = new Timer(WAIT_MS, WAITS);
        waiting.addEventListener(TimerEvent.TIMER, function(event:TimerEvent):void {
            var login:* = gameClass().objLogin;
            if (login == null) {
                return;
            }
            Timer(event.target).stop();
            waiting = null;
            connectChecked(server, login);
        });
        waiting.start();
    }

    /** The stock server list's checks, then the connection. */
    private function connectChecked(server:Object, login:*):void {
        var game:* = gameOf();
        if (server.bOnline == 0) {
            game.MsgBox.notify("Server currently offline!");
        } else if (int(server.iCount) >= int(server.iMax)) {
            game.MsgBox.notify("Server is Full!");
        } else if (int(server.iChat) > 0 && login.bCCOnly == 1) {
            game.MsgBox.notify("Account Restricted to Moglin Sage Server Only.");
        } else if (int(server.iChat) > 0 && int(login.iAge) < 13 && int(login.iUpgDays) < 0) {
            game.MsgBox.notify("Ask your parent to upgrade your account in order to play on chat enabled servers.");
        } else if (server.bUpg == 1 && int(login.iUpgDays) < 0) {
            modal("Member Server! Do you want to upgrade your account to access this premium server now?", "white,medium", "dual");
        } else if (int(server.iMax) % 2 > 0) {
            modal("Testing Server! Do you want to switch to the testing game client?", "white,medium", "dual");
        } else if (int(server.iLevel) > 0 && int(login.iEmailStatus) <= 2) {
            modal("This server requires a confirmed email address.", "red,medium", "mono");
        } else {
            game.objServerInfo = server;
            game.chatF.iChat = server.iChat;
            killLoginModals();
            game.connectTo(server.sIP, int(server.iPort));
        }
    }

    /** A dialog on the login screen, as the stock list opens one (its buttons do nothing here). */
    private function modal(text:String, glow:String, buttons:String):void {
        var game:* = gameOf();
        var ModalClass:Class = Class(game.loaderInfo.applicationDomain.getDefinition("ModalMC"));
        var dialog:* = new ModalClass();
        game.mcLogin.ModalStack.addChild(dialog);
        dialog.init({strBody: text, params: {}, glow: glow, btns: buttons});
    }

    /** Closes every dialog on the login screen, the options menu and in game, and the warning. */
    private function killLoginModals():void {
        var game:* = gameOf();
        for each (var reach:Function in [
            function():* { return game.mcLogin.ModalStack; },
            function():* { return game.mcO.ModalStack; },
            function():* { return game.ui.ModalStack; },
            function():* { return game.ui.mcPopup; }
        ]) {
            var stack:DisplayObjectContainer = null;
            try {
                stack = reach() as DisplayObjectContainer;
            } catch (e:Error) {
            }
            if (stack == null) {
                continue;
            }
            while (stack.numChildren > 0) {
                var child:* = stack.getChildAt(0);
                try {
                    child.fClose();
                } catch (e:Error) {
                }
                if (child.parent == stack) {
                    stack.removeChild(child);
                }
            }
        }
        try {
            game.MsgBox.fClose();
        } catch (e:Error) {
        }
        try {
            game.mcLogin.warning.visible = false;
        } catch (e:Error) {
        }
        try {
            game.serialCmdMode = false;
        } catch (e:Error) {
        }
    }

    /** Stops a connection under way and leaves the server. */
    private function resetConnectionState():void {
        if (waiting != null) {
            waiting.stop();
            waiting = null;
        }
        killLoginModals();
        try {
            gameOf().sfc.disconnect();
        } catch (e:Error) {
        }
    }

    private function loadPolicyFile(ip:*, port:*):void {
        Security.loadPolicyFile("xmlsocket://" + ip + ":" + port);
    }

    /** The policy file of every server the game knows (`serialCmd.servers`) and the list shows. */
    private function loadPolicyFiles():void {
        var seen:Object = {};
        var servers:Array = [];
        try {
            for each (var known:* in gameOf().serialCmd.servers) {
                servers.push(known);
            }
        } catch (e:Error) {
        }
        for each (var entry:* in serverEntries()) {
            try {
                servers.push(entry.obj);
            } catch (e:Error) {
            }
        }
        for each (var server:* in servers) {
            if (server == null || server.sIP == null) {
                continue;
            }
            var key:String = server.sIP + ":" + server.iPort;
            if (seen[key] !== true) {
                seen[key] = true;
                loadPolicyFile(server.sIP, server.iPort);
            }
        }
    }
}
}
