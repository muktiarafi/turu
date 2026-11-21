package turu.host {

import flash.display.DisplayObject;
import flash.display.DisplayObjectContainer;
import flash.text.TextField;
import flash.text.TextFieldType;

/**
 * The Streamer Mode module (docs/VIBESKUA_SPEC.md, "StreamerMode"; Turu's UI.md): your name
 * and guild drawn as "TuruUser" / "Hidden" (never `objData`, which shops and the inventory
 * read), the gold, coins, chat log and your portrait's name hidden, the area title without the
 * room number, a house's sign as "TuruUser's House", and the target portrait's name covered
 * while it names you. Checked on every frame, and each value written only when the game has
 * changed it, so a name the game redraws shows for no frame (VibeSkua's 500 ms loop, and the
 * engine's 250 ms one before this, let it through). Off, it gives back what it changed.
 *
 * At the login screen (no avatar yet), every typing field there that shows its text, the
 * remembered username's above all, is drawn as a password field is (Turu's; VibeSkua leaves the
 * login screen as it is). The text stays, so logging in, by hand or by the bot, works as before.
 *
 * Unlike VibeSkua, the chat's entry field (`mcInterface.te`) stays: hiding a field takes its
 * focus, which took it from you as you typed and closed Android's keyboard. Monsters and other
 * players keep their names in the target portrait (VibeSkua writes "Hidden" over all of them).
 */
public class Streamer {
    public static const NAME:String = "TuruUser";
    public static const GUILD:String = "Hidden";
    public static const HOUSE:String = "TuruUser's House";
    /** Under `ui.mcInterface`: the gold, coins and chat log, in every spelling the interface has
     * had (VibeSkua's), and `mcGold`, the gold as the interface has it now. */
    private static const HIDDEN:Array = [
        "mcGold", "teGold", "teCoins", "strGold", "strCoins", "txtGold", "txtCoins", "t1"
    ];

    private var gameOf:Function;
    private var on:Boolean = false;
    /** The house sign's own text, to give back. */
    private var houseTitle:String;
    /** The login screen's fields drawn as passwords here, to give back. */
    private var masked:Array = [];

    public function Streamer(gameOf:Function) {
        this.gameOf = gameOf;
    }

    public function get enabled():Boolean {
        return on;
    }

    public function set enabled(value:Boolean):void {
        if (value == on) {
            return;
        }
        on = value;
        if (on) {
            frame();
        } else {
            restore();
        }
    }

    /** Every frame while on: what the game changed back is covered again. */
    public function frame():void {
        var game:* = gameOf();
        var w:* = game != null ? game.world : null;
        if (w == null || w.myAvatar == null) {
            loginFrame(game);
            return;
        }
        var me:* = w.myAvatar;
        try {
            setText(me.pMC.pname.ti, NAME);
            setText(me.pMC.pname.tg, GUILD);
        } catch (e:Error) {
        }
        var ui:* = game.ui;
        try {
            var portrait:* = ui.mcPortrait.strName;
            setText(portrait, NAME);
            // The game rewrites it whenever your bars change, but never its visibility.
            portrait.visible = false;
        } catch (e:Error) {
        }
        try {
            var target:* = ui.mcPortraitTarget.strName;
            if (target.text != NAME && sameName(target.text, username(me))) {
                target.text = NAME;
            }
        } catch (e:Error) {
        }
        for each (var name:String in HIDDEN) {
            try {
                var shown:* = ui.mcInterface[name];
                if (shown != null && shown.visible) {
                    shown.visible = false;
                }
            } catch (e:Error) {
            }
        }
        try {
            var map:String = w.strMapName;
            if (map != null && map != "" && map != "null") {
                setText(ui.mcInterface.areaList.title.t1, map.charAt(0).toUpperCase() + map.substr(1).toLowerCase());
            }
        } catch (e:Error) {
        }
        try {
            var sign:* = w.map.txtHouse;
            if (sign != null && sign.text != HOUSE) {
                houseTitle = sign.text;
                sign.text = HOUSE;
            }
        } catch (e:Error) {
        }
    }

    /** At the login screen: its typing fields that show their text drawn as passwords. */
    private function loginFrame(game:*):void {
        var login:DisplayObjectContainer;
        try {
            login = game != null ? game.mcLogin as DisplayObjectContainer : null;
        } catch (e:Error) {
        }
        if (login != null && login.stage != null) {
            maskFields(login, 0);
        }
    }

    private function maskFields(parent:DisplayObjectContainer, depth:int):void {
        // The login screen is a few levels deep; a bound keeps a stray tree from costing a frame.
        if (depth > 8) {
            return;
        }
        for (var i:int = 0; i < parent.numChildren; i++) {
            var child:DisplayObject;
            try {
                child = parent.getChildAt(i);
            } catch (e:Error) {
                continue;
            }
            var field:TextField = child as TextField;
            if (field != null) {
                if (field.type == TextFieldType.INPUT && !field.displayAsPassword) {
                    field.displayAsPassword = true;
                    masked.push(field);
                }
            } else if (child is DisplayObjectContainer) {
                maskFields(DisplayObjectContainer(child), depth + 1);
            }
        }
    }

    /** Off: the hidden parts shown, and the names written back from `objData`. */
    private function restore():void {
        for each (var field:TextField in masked) {
            try {
                field.displayAsPassword = false;
            } catch (e:Error) {
            }
        }
        masked = [];
        var game:* = gameOf();
        var w:* = game != null ? game.world : null;
        if (w == null) {
            return;
        }
        var ui:* = game.ui;
        try {
            if (houseTitle != null && w.map.txtHouse.text == HOUSE) {
                w.map.txtHouse.text = houseTitle;
            }
        } catch (e:Error) {
        }
        houseTitle = null;
        for each (var hidden:String in HIDDEN) {
            try {
                if (ui.mcInterface[hidden] != null) {
                    ui.mcInterface[hidden].visible = true;
                }
            } catch (e:Error) {
            }
        }
        try {
            ui.mcPortrait.strName.visible = true;
        } catch (e:Error) {
        }
        var me:* = w.myAvatar;
        if (me == null) {
            return;
        }
        var name:String = username(me);
        if (name != "") {
            try {
                me.pMC.pname.ti.text = name;
            } catch (e:Error) {
            }
            try {
                ui.mcPortrait.strName.text = name;
            } catch (e:Error) {
            }
            try {
                if (ui.mcPortraitTarget.strName.text == NAME) {
                    ui.mcPortraitTarget.strName.text = name;
                }
            } catch (e:Error) {
            }
        }
        try {
            var guild:* = me.objData.guild;
            me.pMC.pname.tg.text = guild != null && guild.Name != null ? String(guild.Name) : "";
        } catch (e:Error) {
        }
        try {
            w.setUserData();
        } catch (e:Error) {
        }
    }

    private static function setText(field:*, text:String):void {
        if (field != null && field.text != text) {
            field.text = text;
        }
    }

    private static function username(me:*):String {
        try {
            var name:* = me.objData.strUsername;
            return name != null ? String(name) : "";
        } catch (e:Error) {
        }
        return "";
    }

    private static function trim(s:String):String {
        return s.replace(/^\s+|\s+$/g, "");
    }

    /** Whether the portrait's `text` is `name` (the game may draw it in capitals). */
    private static function sameName(text:*, name:String):Boolean {
        return text != null && name != "" && trim(String(text)).toLowerCase() == trim(name).toLowerCase();
    }
}
}
