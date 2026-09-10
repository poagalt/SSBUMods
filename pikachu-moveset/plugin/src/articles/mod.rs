// Neutral B — Piranha Plant's Ptooie, whose spiked ball is the `spikeball` article.
//
// Recipe: ssbu-mod-workshop/episode46/ArticleCloningGuide.md (PhazoGanon & LilyLambda).
// Pikachu has exactly seven vanilla articles (cloud, dengeki, dengekidama, kaminari,
// monsterball, specialupdummy, vortex) per ultimate-research/param-labels articles/Labels.txt,
// so 0x7 is the first free index.
//
// INCOMPLETE: this registers the article and ships its assets. Nothing spawns it yet --
// that needs Piranha Plant's special_n status ported, which is the same class of untestable
// status work as Revenge. Registering an article that is never generated is inert.
use smashline::*;

pub const FIGHTER_PIKACHU_GENERATE_ARTICLE_SPIKEBALL: i32 = 0x7;

pub fn install() {
    smashline::clone_weapon(
        "packun",
        *smash::lib::lua_const::WEAPON_KIND_PACKUN_SPIKEBALL,
        "pikachu",
        "spikeball",
        true,
    );
}
