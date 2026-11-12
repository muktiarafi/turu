package turu.host {
import flash.display.Loader;
import flash.events.TimerEvent;
import flash.utils.Timer;

/**
 * The login screen's background (`setBackgroundValues(sBG, custom)`): `sBG`, the game's own
 * background file, goes into the client's `params`; a custom SWF, if given, is loaded into the
 * login screen's title (`mcLogin.mcTitle`) once it appears, or given up on after 10 s.
 */
public class Background {
    private static const TRIES:int = 40;

    /** The game's background file, `params.sBG`. */
    public var sBG:String = "hideme.swf";

    private var host:Host;
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var custom:String = "";
    private var waiting:Timer;
    private var shown:Loader;

    public function Background(host:Host, gameOf:Function) {
        this.host = host;
        this.gameOf = gameOf;
    }

    public function setValues(sBG:*, custom:*):void {
        if (sBG) {
            this.sBG = String(sBG);
        }
        this.custom = custom ? String(custom) : "";
        var game:* = gameOf();
        if (game != null && game.params != null) {
            game.params.sBG = this.sBG;
        }
        if (waiting != null) {
            waiting.stop();
            waiting = null;
        }
        if (this.custom != "") {
            waiting = new Timer(250, TRIES);
            waiting.addEventListener(TimerEvent.TIMER, onTry);
            waiting.addEventListener(TimerEvent.TIMER_COMPLETE, onGiveUp);
            waiting.start();
        }
    }

    private function onTry(event:TimerEvent):void {
        var title:* = null;
        try {
            title = gameOf().mcLogin.mcTitle;
        } catch (e:Error) {
        }
        if (title == null) {
            return;
        }
        waiting.stop();
        waiting = null;
        if (shown != null && shown.parent != null) {
            shown.parent.removeChild(shown);
        }
        shown = new Loader();
        host.load(shown, custom);
        title.addChild(shown);
    }

    private function onGiveUp(event:TimerEvent):void {
        waiting = null;
        host.call("debug", ["No login screen for the custom background " + custom]);
    }
}
}
