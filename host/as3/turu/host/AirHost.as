package turu.host {
import flash.display.DisplayObject;
import flash.display.Loader;
import flash.system.ApplicationDomain;

/**
 * The [Host] under Turu Mas (AIR, docs/TURU_MAS.md): the AIR host SWF (`air/app`) compiles in
 * `TuruHost`, whose static methods take this SWF's tab (`client`, from the host's
 * `LoaderContext` parameters, `turuClient`). Every tab shares AIR's one stage, so the host
 * places the game, keeps its frame rate and scale, and says which tab is on screen.
 * `TuruHost` is reached by name: this SWF is built apart from the AIR host's.
 */
public class AirHost extends Host {
    private var client:int;
    private var turuHost:Object;

    public function AirHost(client:int) {
        super();
        this.client = client;
        turuHost = ApplicationDomain.currentDomain.getDefinition("TuruHost");
    }

    override public function addCallback(name:String, fn:Function):void {
        turuHost.addCallback(client, name, fn);
    }

    override public function call(name:String, args:Array):void {
        turuHost.call(client, name, args);
    }

    /**
     * Into a domain under this SWF's, as VibeSkua's loads its game: `TuruStage` then knows the
     * tab from the game's domain (this SWF's class is in it) even while the game is still being
     * built, before its loader's `init`.
     */
    override public function load(url:String):Loader {
        return Loader(turuHost.load(client, url, new ApplicationDomain(ApplicationDomain.currentDomain)));
    }

    override public function show(main:DisplayObject, game:DisplayObject):void {
        turuHost.showGame(client, main, game);
    }

    override public function isShown():Boolean {
        return turuHost.isShown(client) == true;
    }

    override public function ownsStage():Boolean {
        return false;
    }
}
}
