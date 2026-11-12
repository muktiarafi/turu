package turu.host {
import flash.display.DisplayObject;
import flash.display.Loader;
import flash.display.Stage;
import flash.display.StageScaleMode;
import flash.external.ExternalInterface;
import flash.net.URLRequest;

/** The [Host] of a player with `ExternalInterface` and a stage of its own: Ruffle's. */
public class ExternalHost extends Host {
    private var stage:Stage;
    /** The game on screen, taken off by reference when another replaces it. */
    private var shown:DisplayObject;

    public function ExternalHost(stage:Stage) {
        super();
        this.stage = stage;
    }

    override public function addCallback(name:String, fn:Function):void {
        ExternalInterface.addCallback(name, fn);
    }

    override public function call(name:String, args:Array):void {
        if (ExternalInterface.available) {
            ExternalInterface.call(name, args);
        }
    }

    override public function load(loader:Loader, url:String):void {
        // No LoaderContext: the game lands in this SWF's domain, as with VibeSkua's.
        loader.load(new URLRequest(url));
    }

    override public function show(main:DisplayObject, game:DisplayObject):void {
        if (main.parent == stage) {
            stage.removeChild(main);
        }
        if (shown != null && shown != game && shown.parent == stage) {
            stage.removeChild(shown);
        }
        shown = game;
        stage.addChild(game);
        stage.scaleMode = StageScaleMode.SHOW_ALL;
        // Centred: VibeSkua's TOP leaves all the empty space under the game.
        stage.align = "";
    }
}
}
