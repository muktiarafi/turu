package turu.host {

/**
 * Monsters, targets and skills (docs/VIBESKUA_SPEC.md, "Monster" and "Skills and combat").
 * A monster's JSON is its `objData` with `intHP`, `intHPMax` and `intState` from its
 * `dataLeaf`.
 */
public class Combat {
    /** Returns the game, or null before it loads. */
    private var gameOf:Function;

    public function Combat(gameOf:Function) {
        this.gameOf = gameOf;
    }

    /** Registers the calls with `expose(name, fn, onThrow, quiet)`; reads are quiet. */
    public function exposeWith(expose:Function):void {
        expose("availableMonsters", availableMonsters, undefined, true);
        expose("getMonsters", getMonsters, undefined, true);
        expose("getTargetMonster", getTargetMonster, undefined, true);
        expose("canUseSkill", canUseSkill, undefined, true);
        expose("snapshot", snapshot, undefined, true);
        expose("attackMonsterName", attackMonsterName);
        expose("attackMonsterID", attackMonsterID);
        expose("attackPlayer", attackPlayer);
        expose("untargetSelf", untargetSelf);
        expose("useSkill", useSkill);
        expose("infiniteRange", infiniteRange);
        expose("magnetize", magnetize);
    }

    private function get world():* {
        return gameOf().world;
    }

    /** The monsters in your cell that are on screen (have a `pMC`). */
    private function inCell():Array {
        var found:Array = [];
        for each (var monster:* in world.getMonstersByCell(world.strFrame)) {
            if (monster != null && monster.pMC != null) {
                found.push(monster);
            }
        }
        return found;
    }

    private static function monsterJson(monster:*):Object {
        var out:Object = {};
        var data:* = monster.objData;
        for (var key:String in data) {
            out[key] = data[key];
        }
        var leaf:* = monster.dataLeaf;
        if (leaf != null) {
            out.intHP = leaf.intHP;
            out.intHPMax = leaf.intHPMax;
            out.intState = leaf.intState;
        }
        return out;
    }

    private static function listJson(monsters:*):String {
        var out:Array = [];
        for each (var monster:* in monsters) {
            if (monster != null) {
                out.push(monsterJson(monster));
            }
        }
        return JSON.stringify(out);
    }

    private function availableMonsters():String {
        return listJson(inCell());
    }

    private function getMonsters():String {
        return listJson(world.monsters);
    }

    /** Your target, or `{}` (and no target) when it's gone or dead. */
    private function getTargetMonster():String {
        var target:* = world.myAvatar.target;
        if (target == null || target.dataLeaf == null || target.dataLeaf.intHP <= 0) {
            world.cancelTarget();
            return "{}";
        }
        return JSON.stringify(monsterJson(target));
    }

    /**
     * What the skill timer and the engine's waits ask on every tick, in one read:
     * `{loggedIn, world, avatar, state, loaded, rank, target}`. `state` is the avatar's
     * `intState` (0 dead or no avatar); `loaded` means items loaded, no map load in progress
     * and the avatar's art loaded; `rank` the class rank (null without one). With
     * `withTarget`, `target` is `getTargetMonster`'s answer, dropping a dead or missing target
     * as it does; otherwise null and the target is left alone.
     */
    private function snapshot(withTarget:*):String {
        var out:Object = {loggedIn: false, world: false, avatar: false, state: 0, loaded: false, rank: null,
            target: null};
        var game:* = gameOf();
        if (game == null) {
            return JSON.stringify(out);
        }
        out.loggedIn = game.sfc != null && game.sfc.isConnected == true;
        var world:* = game.world;
        if (world == null) {
            return JSON.stringify(out);
        }
        out.world = true;
        var avatar:* = world.myAvatar;
        if (avatar != null) {
            out.avatar = true;
            try {
                out.state = int(avatar.dataLeaf.intState);
            } catch (e:Error) {
            }
            out.loaded = loaded(world, avatar);
            try {
                out.rank = avatar.objData.iRank;
            } catch (e:Error) {
            }
        }
        if (withTarget == true && avatar != null) {
            var target:* = avatar.target;
            if (target == null || target.dataLeaf == null || target.dataLeaf.intHP <= 0) {
                world.cancelTarget();
            } else {
                out.target = monsterJson(target);
            }
        }
        return JSON.stringify(out);
    }

    private static function loaded(world:*, avatar:*):Boolean {
        try {
            if (avatar.items == null || avatar.items.length <= 0 || world.mapLoadInProgress !== false) {
                return false;
            }
            return avatar.pMC.artLoaded() === true;
        } catch (e:Error) {
        }
        return false;
    }

    private function attackMonsterName(name:*):String {
        var wanted:String = String(name).toLowerCase();
        return attack(function(monster:*):Boolean {
            return wanted == "*" || String(monster.objData.strMonName).toLowerCase().indexOf(wanted) >= 0;
        });
    }

    private function attackMonsterID(id:*):String {
        var wanted:Number = Number(id);
        return attack(function(monster:*):Boolean {
            return monster.objData.MonMapID == wanted || monster.objData.MonID == wanted;
        });
    }

    /** Targets the best of your cell's monsters that `matches`: alive, least HP, lowest map ID. */
    private function attack(matches:Function):String {
        var candidates:Array = inCell().filter(function(monster:*, i:int, all:Array):Boolean {
            return matches(monster);
        });
        if (candidates.length == 0) {
            return "false";
        }
        candidates.sort(function(a:*, b:*):int {
            var aliveA:Boolean = hp(a) > 0;
            var aliveB:Boolean = hp(b) > 0;
            if (aliveA != aliveB) {
                return aliveA ? -1 : 1;
            }
            if (hp(a) != hp(b)) {
                return hp(a) < hp(b) ? -1 : 1;
            }
            var idA:Number = Number(a.objData.MonMapID);
            var idB:Number = Number(b.objData.MonMapID);
            return idA < idB ? -1 : (idA > idB ? 1 : 0);
        });
        world.setTarget(candidates[0]);
        world.approachTarget();
        return "true";
    }

    private static function hp(monster:*):Number {
        return monster.dataLeaf != null ? Number(monster.dataLeaf.intHP) : 0;
    }

    private function attackPlayer(name:*):void {
        var avatar:* = world.getAvatarByUserName(String(name));
        if (avatar != null) {
            world.setTarget(avatar);
            world.approachTarget();
        }
    }

    private function untargetSelf():void {
        if (world.myAvatar.target == world.myAvatar) {
            world.cancelTarget();
        }
    }

    /** Whether skill `index` can be used now: a live target, and no cooldown or lock. */
    private function canUseSkill(index:*):String {
        var target:* = world.myAvatar.target;
        if (target == null || target.dataLeaf == null || target.dataLeaf.intHP <= 0) {
            return "false";
        }
        var skill:* = world.actions.active[int(index)];
        return String(skill != null && cooledDown(skill) && skill.isOK && !skill.skillLock && !skill.lock);
    }

    /** The global cooldown and the skill's own (scaled by haste) have passed. */
    private function cooledDown(skill:*):Boolean {
        var now:Number = new Date().getTime();
        if (now < Number(world.GCDTS) + Number(world.GCD)) {
            return false;
        }
        var haste:Number = 1 - Math.min(Math.max(Number(world.myAvatar.dataLeaf.sta["$tha"]), -1), 0.5);
        var cooldown:Number = Number(skill.OldCD != null ? skill.OldCD : skill.cd);
        return now >= Number(skill.ts) + cooldown * haste;
    }

    private function useSkill(index:*):void {
        var skill:* = world.actions.active[int(index)];
        if (skill != null && cooledDown(skill)) {
            world.testAction(skill);
        }
    }

    /** Every skill reaches across the map. */
    private function infiniteRange():void {
        var active:* = world.actions.active;
        if (active == null) {
            return;
        }
        for (var i:int = 0; i < 6; i++) {
            if (active[i] != null) {
                active[i].range = 20000;
            }
        }
    }

    /** Your target moves to you. */
    private function magnetize():void {
        var target:* = world.myAvatar.target;
        var me:* = world.myAvatar.pMC;
        if (target != null && target.pMC != null && me != null) {
            target.pMC.x = me.x;
            target.pMC.y = me.y;
        }
    }
}
}
