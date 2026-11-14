package turu.host {
import flash.display.DisplayObject;
import flash.display.Loader;
import flash.display.Stage;
import flash.display.StageScaleMode;
import flash.external.ExternalInterface;
import flash.net.URLRequest;

/**
 * The [Host] of a player with `ExternalInterface` and a stage of its own: Ruffle's (desktop and
 * Ruffle Android).
 */
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
            ExternalInterface.call.apply(ExternalInterface, [name].concat(args));
        }
    }

    override public function load(url:String):Loader {
        // No LoaderContext: as VibeSkua's, the game gets a domain under this SWF's.
        var loader:Loader = new Loader();
        loader.load(new URLRequest(url));
        return loader;
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
