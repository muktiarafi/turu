package turu.host {
import flash.display.Bitmap;
import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.display.Loader;
import flash.events.Event;
import flash.events.IOErrorEvent;
import flash.events.TimerEvent;
import flash.geom.Point;
import flash.utils.ByteArray;
import flash.utils.Timer;

/**
 * The login screen's background (`setBackgroundValues(sBG, custom)`): `sBG`, the game's own
 * background file, goes into the client's `params`; a custom SWF, if given, is loaded into the
 * login screen's title (`mcLogin.mcTitle`) once it appears, or given up on after 10 s.
 *
 * Turu's own login picture (`setLoginImage`, a JPEG or PNG the bot sends) goes over the game's
 * background in the title, sized to the game's area, each time a login screen is there (it's made
 * anew after a logout), and back on top of the title once the game's background loads in after it.
 */
public class Background {
    private static const TRIES:int = 40;

    /** The game's own size, which the login picture covers. */
    private static const WIDTH:Number = 960;
    private static const HEIGHT:Number = 550;

    /**
     * The game's background file, `params.sBG`: the one the bot names (`Generic2.swf`) from the
     * start, as the bot names it only a second after the client is loading. VibeSkua's
     * `hideme.swf` (none) left the login screen black when the client built it before then.
     */
    public var sBG:String = "Generic2.swf";

    private var host:Host;
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;
    private var custom:String = "";
    private var waiting:Timer;
    private var shown:Loader;

    /** The login picture, once loaded ([setImage]). */
    private var picture:Loader;
    private var pictureReady:Boolean = false;
    /** Whether the first login screen's `sBG` was reported. */
    private var titleSeen:Boolean = false;

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

    /** `setLoginImage(base64)`: the login picture's bytes (JPEG or PNG), base64. */
    public function setImage(base64:*):void {
        var bytes:ByteArray = decode(String(base64));
        if (bytes.length == 0) {
            return;
        }
        if (picture != null && picture.parent != null) {
            picture.parent.removeChild(picture);
        }
        pictureReady = false;
        var loader:Loader = new Loader();
        // It's a picture: taps go through to the login form.
        loader.mouseEnabled = false;
        loader.mouseChildren = false;
        loader.contentLoaderInfo.addEventListener(Event.COMPLETE, function(event:Event):void {
            var bitmap:Bitmap = loader.content as Bitmap;
            if (bitmap != null) {
                // Turu Mas draws the games smaller with several tabs.
                bitmap.smoothing = true;
            }
            pictureReady = true;
            host.call("debug", ["Login picture: " + loader.content.width + "x" + loader.content.height]);
        });
        loader.contentLoaderInfo.addEventListener(IOErrorEvent.IO_ERROR, function(event:IOErrorEvent):void {
            host.call("debug", ["The login picture couldn't be read: " + event.text]);
        });
        picture = loader;
        loader.loadBytes(bytes);
    }

    /**
     * Each game frame: the login picture into the login screen's title when there is one and it
     * isn't there yet, over the game's background and fitted to the game's area.
     */
    public function frame():void {
        var game:* = gameOf();
        var title:DisplayObjectContainer = null;
        try {
            title = game.mcLogin.mcTitle as DisplayObjectContainer;
        } catch (e:Error) {
        }
        if (title == null) {
            return;
        }
        if (!titleSeen) {
            titleSeen = true;
            host.call("debug", ["Login screen: sBG " + (game.params != null ? game.params.sBG : "?")]);
        }
        if (!pictureReady) {
            return;
        }
        if (picture.parent == title) {
            // The game loads its own background into the title after: the picture stays over it.
            if (title.getChildIndex(picture) != title.numChildren - 1) {
                title.setChildIndex(picture, title.numChildren - 1);
            }
            return;
        }
        var root:DisplayObject = game as DisplayObject;
        var topLeft:Point = title.globalToLocal(root.localToGlobal(new Point(0, 0)));
        var bottomRight:Point = title.globalToLocal(root.localToGlobal(new Point(WIDTH, HEIGHT)));
        title.addChild(picture);
        picture.x = topLeft.x;
        picture.y = topLeft.y;
        picture.width = bottomRight.x - topLeft.x;
        picture.height = bottomRight.y - topLeft.y;
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
        shown = host.load(custom);
        title.addChild(shown);
    }

    private function onGiveUp(event:TimerEvent):void {
        waiting = null;
        host.call("debug", ["No login screen for the custom background " + custom]);
    }

    private static const ALPHABET:String = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    /** Base64 to bytes (Flash has no decoder of its own); what isn't base64 is skipped. */
    private static function decode(text:String):ByteArray {
        var out:ByteArray = new ByteArray();
        var buffer:int = 0;
        var bits:int = 0;
        for (var i:int = 0; i < text.length; i++) {
            var value:int = ALPHABET.indexOf(text.charAt(i));
            if (value < 0) {
                continue;
            }
            buffer = (buffer << 6) | value;
            bits += 6;
            if (bits >= 8) {
                bits -= 8;
                out.writeByte((buffer >> bits) & 0xFF);
            }
        }
        out.position = 0;
        return out;
    }
}
}
