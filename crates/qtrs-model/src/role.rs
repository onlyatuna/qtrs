//! Item roles, item flags and related enumerations (`Qt::ItemDataRole`,
//! `Qt::ItemFlags`, `Qt::Orientation`, `Qt::SortOrder`, `Qt::MatchFlags`, ...).

/// Generates the bit operators and helpers for a `pub struct Name(pub u32)` flag set.
macro_rules! impl_flags {
    ($name:ident) => {
        impl $name {
            /// Returns the raw bit value.
            #[inline]
            pub const fn bits(self) -> u32 {
                self.0
            }

            /// Builds a flag set from raw bits.
            #[inline]
            pub const fn from_bits(bits: u32) -> Self {
                Self(bits)
            }

            /// Returns `true` if every bit of `other` is set in `self`.
            #[inline]
            pub const fn contains(self, other: Self) -> bool {
                (self.0 & other.0) == other.0
            }

            /// Returns `true` if any bit of `other` is set in `self`.
            #[inline]
            pub const fn intersects(self, other: Self) -> bool {
                (self.0 & other.0) != 0
            }

            /// Returns `true` if no bit is set.
            #[inline]
            pub const fn is_empty(self) -> bool {
                self.0 == 0
            }

            /// Sets the bits of `other`.
            #[inline]
            pub fn insert(&mut self, other: Self) {
                self.0 |= other.0;
            }

            /// Clears the bits of `other`.
            #[inline]
            pub fn remove(&mut self, other: Self) {
                self.0 &= !other.0;
            }

            /// Sets or clears the bits of `other` depending on `on`.
            #[inline]
            pub fn set(&mut self, other: Self, on: bool) {
                if on {
                    self.insert(other);
                } else {
                    self.remove(other);
                }
            }
        }

        impl std::ops::BitOr for $name {
            type Output = Self;
            #[inline]
            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }

        impl std::ops::BitOrAssign for $name {
            #[inline]
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl std::ops::BitAnd for $name {
            type Output = Self;
            #[inline]
            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }

        impl std::ops::BitAndAssign for $name {
            #[inline]
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }

        impl std::ops::BitXor for $name {
            type Output = Self;
            #[inline]
            fn bitxor(self, rhs: Self) -> Self {
                Self(self.0 ^ rhs.0)
            }
        }

        impl std::ops::Not for $name {
            type Output = Self;
            #[inline]
            fn not(self) -> Self {
                Self(!self.0)
            }
        }
    };
}
pub(crate) use impl_flags;

/// Data roles understood by item models (`Qt::ItemDataRole`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemDataRole {
    Display,
    Decoration,
    Edit,
    ToolTip,
    StatusTip,
    WhatsThis,
    Font,
    TextAlignment,
    Background,
    Foreground,
    CheckState,
    SizeHint,
    /// Application specific role: `Qt::UserRole + n`.
    User(u32),
}

impl ItemDataRole {
    /// First user role (`Qt::UserRole`).
    pub const USER: ItemDataRole = ItemDataRole::User(0);

    /// Numeric value matching Qt's role numbering.
    pub const fn to_i32(self) -> i32 {
        match self {
            ItemDataRole::Display => 0,
            ItemDataRole::Decoration => 1,
            ItemDataRole::Edit => 2,
            ItemDataRole::ToolTip => 3,
            ItemDataRole::StatusTip => 4,
            ItemDataRole::WhatsThis => 5,
            ItemDataRole::Font => 6,
            ItemDataRole::TextAlignment => 7,
            ItemDataRole::Background => 8,
            ItemDataRole::Foreground => 9,
            ItemDataRole::CheckState => 10,
            ItemDataRole::SizeHint => 13,
            ItemDataRole::User(n) => 0x0100 + n as i32,
        }
    }

    /// Converts a Qt role number back into a role.
    pub const fn from_i32(value: i32) -> Option<Self> {
        Some(match value {
            0 => ItemDataRole::Display,
            1 => ItemDataRole::Decoration,
            2 => ItemDataRole::Edit,
            3 => ItemDataRole::ToolTip,
            4 => ItemDataRole::StatusTip,
            5 => ItemDataRole::WhatsThis,
            6 => ItemDataRole::Font,
            7 => ItemDataRole::TextAlignment,
            8 => ItemDataRole::Background,
            9 => ItemDataRole::Foreground,
            10 => ItemDataRole::CheckState,
            13 => ItemDataRole::SizeHint,
            v if v >= 0x0100 => ItemDataRole::User((v - 0x0100) as u32),
            _ => return None,
        })
    }

    /// Roles queried by the default `item_data` implementation.
    pub const STANDARD_ROLES: [ItemDataRole; 12] = [
        ItemDataRole::Display,
        ItemDataRole::Decoration,
        ItemDataRole::Edit,
        ItemDataRole::ToolTip,
        ItemDataRole::StatusTip,
        ItemDataRole::WhatsThis,
        ItemDataRole::Font,
        ItemDataRole::TextAlignment,
        ItemDataRole::Background,
        ItemDataRole::Foreground,
        ItemDataRole::CheckState,
        ItemDataRole::SizeHint,
    ];
}

impl From<ItemDataRole> for i32 {
    fn from(role: ItemDataRole) -> i32 {
        role.to_i32()
    }
}

/// Item capability flags (`Qt::ItemFlags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ItemFlags(pub u32);

impl ItemFlags {
    pub const NONE: Self = Self(0);
    pub const SELECTABLE: Self = Self(1);
    pub const EDITABLE: Self = Self(2);
    pub const DRAG_ENABLED: Self = Self(4);
    pub const DROP_ENABLED: Self = Self(8);
    pub const USER_CHECKABLE: Self = Self(16);
    pub const ENABLED: Self = Self(32);
    pub const AUTO_TRISTATE: Self = Self(64);
    pub const NEVER_HAS_CHILDREN: Self = Self(128);
    pub const USER_TRISTATE: Self = Self(256);
}
impl_flags!(ItemFlags);

/// Orientation of headers and sections (`Qt::Orientation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Sort direction (`Qt::SortOrder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

/// Check state stored under [`ItemDataRole::CheckState`] (`Qt::CheckState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CheckState {
    #[default]
    Unchecked,
    PartiallyChecked,
    Checked,
}

impl CheckState {
    /// Numeric value matching Qt (0, 1, 2).
    pub const fn to_i32(self) -> i32 {
        match self {
            CheckState::Unchecked => 0,
            CheckState::PartiallyChecked => 1,
            CheckState::Checked => 2,
        }
    }

    /// Converts Qt's numeric check state.
    pub const fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(CheckState::Unchecked),
            1 => Some(CheckState::PartiallyChecked),
            2 => Some(CheckState::Checked),
            _ => None,
        }
    }
}

impl From<CheckState> for qtrs_core::Variant {
    fn from(state: CheckState) -> Self {
        qtrs_core::Variant::I64(state.to_i32() as i64)
    }
}

/// String comparison mode (`Qt::CaseSensitivity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CaseSensitivity {
    Insensitive,
    #[default]
    Sensitive,
}

/// Matching behaviour for [`crate::AbstractItemModel::match_indexes`] (`Qt::MatchFlags`).
///
/// The low four bits select the match type; the remaining bits are modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MatchFlags(pub u32);

impl MatchFlags {
    pub const EXACTLY: Self = Self(0);
    pub const CONTAINS: Self = Self(1);
    pub const STARTS_WITH: Self = Self(2);
    pub const ENDS_WITH: Self = Self(3);
    pub const REGULAR_EXPRESSION: Self = Self(4);
    pub const WILDCARD: Self = Self(5);
    pub const FIXED_STRING: Self = Self(8);
    pub const TYPE_MASK: Self = Self(0x0F);
    pub const CASE_SENSITIVE: Self = Self(16);
    pub const WRAP: Self = Self(32);
    pub const RECURSIVE: Self = Self(64);

    /// The match type part (`flags & MatchTypeMask`).
    pub const fn match_type(self) -> Self {
        Self(self.0 & 0x0F)
    }
}
impl_flags!(MatchFlags);
