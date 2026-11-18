package turu.host {
import flash.display.BitmapData;

/**
 * Turu's login picture (`host/assets/login-background.jpg`, 960x550, the game's size): a bitmap
 * symbol of turu.swf linked to this class, as Flash Pro links a library bitmap. The builder
 * (`host/src/main.rs`) writes the picture's tag; asc has no `[Embed]`. The size given is ignored
 * for a linked symbol.
 */
public class LoginPicture extends BitmapData {
    public function LoginPicture(width:int = 960, height:int = 550) {
        super(width, height);
    }
}
}
