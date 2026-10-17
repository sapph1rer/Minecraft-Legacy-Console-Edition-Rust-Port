//! Port of `Material`, `MaterialColor` and the Gas/Liquid/Decoration/Portal/Web subclasses.
//!
//! The C++ builds ~33 heap objects in `Material::staticCtor` with a fluent builder; here each
//! material is an enum variant and its flags live in a `const` table built with the same
//! builder-style `const fn`s, so the table reads like the original constructor list.
//!
//! `MaterialColor` keeps only the *id* (0..=13). The actual map colours come from the texture
//! pack's colour table (`eMinecraftColour_Material_*`), which is a client resource.

/// `MaterialColor::*` (id order as in the original).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaterialColor {
    None = 0,
    Grass = 1,
    Sand = 2,
    Cloth = 3,
    Fire = 4,
    Ice = 5,
    Metal = 6,
    Plant = 7,
    Snow = 8,
    Clay = 9,
    Dirt = 10,
    Stone = 11,
    Water = 12,
    Wood = 13,
}

impl MaterialColor {
    pub const fn id(self) -> u8 {
        self as u8
    }
}

/// `Material::PUSH_*`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PushReaction {
    Normal = 0,
    Destroy = 1,
    /// not pushable
    Block = 2,
}

/// Which C++ subclass a material is (decides `isSolid` / `blocksLight` / `blocksMotion` / `isLiquid`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Plain,
    Gas,
    Liquid,
    Decoration,
    Portal,
    Web,
}

#[derive(Clone, Copy, Debug)]
struct Def {
    kind: Kind,
    color: MaterialColor,
    flammable: bool,
    replaceable: bool,
    never_buildable: bool,
    always_destroyable: bool,
    push: PushReaction,
    destroyed_by_hand: bool,
}

impl Def {
    /// `new Material(color)` and the subclass constructors (which already apply
    /// `replaceable()`, `destroyOnPush()`, `makeDestroyedByHand()` where the C++ does).
    const fn new(kind: Kind, color: MaterialColor) -> Def {
        let mut d = Def {
            kind,
            color,
            flammable: false,
            replaceable: false,
            never_buildable: false,
            always_destroyable: true,
            push: PushReaction::Normal,
            destroyed_by_hand: false,
        };
        match kind {
            Kind::Gas => d.replaceable = true,
            Kind::Liquid => {
                d.replaceable = true;
                d.push = PushReaction::Destroy;
            }
            Kind::Decoration => d.destroyed_by_hand = true,
            _ => {}
        }
        d
    }
    const fn flammable(mut self) -> Def {
        self.flammable = true;
        self
    }
    const fn replaceable(mut self) -> Def {
        self.replaceable = true;
        self
    }
    const fn never_buildable(mut self) -> Def {
        self.never_buildable = true;
        self
    }
    const fn not_always_destroyable(mut self) -> Def {
        self.always_destroyable = false;
        self
    }
    const fn destroy_on_push(mut self) -> Def {
        self.push = PushReaction::Destroy;
        self
    }
    const fn not_pushable(mut self) -> Def {
        self.push = PushReaction::Block;
        self
    }
    const fn destroyed_by_hand(mut self) -> Def {
        self.destroyed_by_hand = true;
        self
    }
}

macro_rules! materials {
    ($( $variant:ident => $def:expr ),+ $(,)?) => {
        /// `Material::*` (declaration order of `Material.h`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u8)]
        pub enum Material { $($variant),+ }

        impl Material {
            /// All materials in declaration order.
            pub const ALL: &'static [Material] = &[ $(Material::$variant),+ ];
        }

        const DEFS: &[Def] = &[ $($def),+ ];
    };
}

use Kind::*;
use MaterialColor as C;

// Mirrors Material::staticCtor line by line.
materials! {
    Air              => Def::new(Gas, C::None),
    Grass            => Def::new(Plain, C::Grass),
    Dirt             => Def::new(Plain, C::Dirt),
    Wood             => Def::new(Plain, C::Wood).flammable(),
    Stone            => Def::new(Plain, C::Stone).not_always_destroyable(),
    Metal            => Def::new(Plain, C::Metal).not_always_destroyable(),
    HeavyMetal       => Def::new(Plain, C::Metal).not_always_destroyable().not_pushable(),
    Water            => Def::new(Liquid, C::Water).destroy_on_push(),
    Lava             => Def::new(Liquid, C::Fire).destroy_on_push(),
    Leaves           => Def::new(Plain, C::Plant).flammable().never_buildable().destroy_on_push(),
    Plant            => Def::new(Decoration, C::Plant).destroy_on_push(),
    ReplaceablePlant => Def::new(Decoration, C::Plant).flammable().destroy_on_push().replaceable(),
    Sponge           => Def::new(Plain, C::Cloth),
    Cloth            => Def::new(Plain, C::Cloth).flammable(),
    Fire             => Def::new(Gas, C::None).destroy_on_push(),
    Sand             => Def::new(Plain, C::Sand),
    Decoration       => Def::new(Decoration, C::None).destroy_on_push(),
    ClothDecoration  => Def::new(Decoration, C::Cloth).flammable(),
    Glass            => Def::new(Plain, C::None).never_buildable().destroyed_by_hand(),
    BuildableGlass   => Def::new(Plain, C::None).destroyed_by_hand(),
    Explosive        => Def::new(Plain, C::Fire).flammable().never_buildable(),
    Coral            => Def::new(Plain, C::Plant).destroy_on_push(),
    Ice              => Def::new(Plain, C::Ice).never_buildable().destroyed_by_hand(),
    TopSnow          => Def::new(Decoration, C::Snow).replaceable().never_buildable().not_always_destroyable().destroy_on_push(),
    Snow             => Def::new(Plain, C::Snow).not_always_destroyable(),
    Cactus           => Def::new(Plain, C::Plant).never_buildable().destroy_on_push(),
    Clay             => Def::new(Plain, C::Clay),
    Vegetable        => Def::new(Plain, C::Plant).destroy_on_push(),
    Egg              => Def::new(Plain, C::Plant).destroy_on_push(),
    Portal           => Def::new(Portal, C::None).not_pushable(),
    Cake             => Def::new(Plain, C::None).destroy_on_push(),
    Web              => Def::new(Web, C::Cloth).not_always_destroyable().destroy_on_push(),
    Piston           => Def::new(Plain, C::Stone).not_pushable(),
}

impl Material {
    fn def(self) -> &'static Def {
        &DEFS[self as usize]
    }

    pub fn color(self) -> MaterialColor {
        self.def().color
    }

    pub fn is_liquid(self) -> bool {
        self.def().kind == Kind::Liquid
    }

    pub fn is_solid(self) -> bool {
        !matches!(self.def().kind, Kind::Gas | Kind::Liquid | Kind::Decoration | Kind::Portal)
    }

    pub fn blocks_light(self) -> bool {
        !matches!(self.def().kind, Kind::Gas | Kind::Decoration | Kind::Portal)
    }

    pub fn blocks_motion(self) -> bool {
        // Web and Liquid also override this to false.
        !matches!(
            self.def().kind,
            Kind::Gas | Kind::Liquid | Kind::Decoration | Kind::Portal | Kind::Web
        )
    }

    pub fn lets_water_through(self) -> bool {
        !self.is_liquid() && !self.is_solid()
    }

    pub fn is_flammable(self) -> bool {
        self.def().flammable
    }

    pub fn is_replaceable(self) -> bool {
        self.def().replaceable
    }

    pub fn is_solid_blocking(self) -> bool {
        if self.def().never_buildable {
            return false;
        }
        self.blocks_motion()
    }

    /// Drops resources regardless of the player's tool.
    pub fn is_always_destroyable(self) -> bool {
        self.def().always_destroyable
    }

    pub fn push_reaction(self) -> PushReaction {
        self.def().push
    }

    pub fn is_destroyed_by_hand(self) -> bool {
        self.def().destroyed_by_hand
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_one_def_per_variant() {
        assert_eq!(DEFS.len(), Material::ALL.len());
        assert_eq!(Material::ALL.len(), 33);
        assert_eq!(Material::ALL[0], Material::Air);
        assert_eq!(*Material::ALL.last().unwrap(), Material::Piston);
    }

    #[test]
    fn air_and_fire_are_gases() {
        for m in [Material::Air, Material::Fire] {
            assert!(!m.is_solid() && !m.blocks_light() && !m.blocks_motion() && !m.is_liquid());
            assert!(m.is_replaceable());
            assert!(m.lets_water_through());
        }
        assert_eq!(Material::Fire.push_reaction(), PushReaction::Destroy);
        assert_eq!(Material::Air.push_reaction(), PushReaction::Normal);
    }

    #[test]
    fn liquids() {
        for m in [Material::Water, Material::Lava] {
            assert!(m.is_liquid());
            assert!(!m.is_solid());
            assert!(!m.blocks_motion());
            assert!(m.blocks_light()); // LiquidMaterial does not override blocksLight
            assert!(m.is_replaceable());
            assert!(!m.lets_water_through()); // liquid
            assert_eq!(m.push_reaction(), PushReaction::Destroy);
        }
        assert_eq!(Material::Water.color(), MaterialColor::Water);
        assert_eq!(Material::Lava.color(), MaterialColor::Fire);
    }

    #[test]
    fn solids() {
        for m in [Material::Stone, Material::Dirt, Material::Sand, Material::Wood, Material::Ice] {
            assert!(m.is_solid() && m.blocks_light() && m.blocks_motion());
        }
        assert!(Material::Stone.is_solid_blocking());
        // neverBuildable materials are not "solid blocking"
        assert!(!Material::Leaves.is_solid_blocking());
        assert!(!Material::Ice.is_solid_blocking());
        assert!(!Material::Glass.is_solid_blocking());
        assert!(Material::BuildableGlass.is_solid_blocking());
        assert!(!Material::Stone.is_always_destroyable());
        assert!(Material::Dirt.is_always_destroyable());
        assert!(Material::Wood.is_flammable() && !Material::Stone.is_flammable());
    }

    #[test]
    fn decoration_and_special() {
        for m in [Material::Plant, Material::ReplaceablePlant, Material::Decoration, Material::ClothDecoration] {
            assert!(!m.is_solid() && !m.blocks_light() && !m.blocks_motion());
            assert!(m.is_destroyed_by_hand());
        }
        assert!(Material::ReplaceablePlant.is_replaceable());
        assert!(Material::TopSnow.is_replaceable());
        assert!(!Material::TopSnow.is_always_destroyable());
        // web: solid and light-blocking, but not motion-blocking
        assert!(Material::Web.is_solid() && Material::Web.blocks_light() && !Material::Web.blocks_motion());
        assert_eq!(Material::Portal.push_reaction(), PushReaction::Block);
        assert_eq!(Material::HeavyMetal.push_reaction(), PushReaction::Block);
        assert_eq!(Material::Piston.push_reaction(), PushReaction::Block);
        assert!(!Material::Portal.is_solid());
    }

    #[test]
    fn material_color_ids() {
        assert_eq!(MaterialColor::None.id(), 0);
        assert_eq!(MaterialColor::Wood.id(), 13);
        assert_eq!(Material::Grass.color().id(), 1);
    }
}
