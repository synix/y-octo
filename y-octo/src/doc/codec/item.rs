use super::*;
use crate::sync::{Arc, AtomicU8, Ordering};

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub(crate) enum Parent {
    #[cfg_attr(test, proptest(skip))]
    Type(YTypeRef),
    String(String),
    Id(Id),
}

#[rustfmt::skip]
#[allow(dead_code)]
pub mod item_flags {
    pub const ITEM_KEEP                 : u8 = 0b0000_0001;
    pub const ITEM_COUNTABLE            : u8 = 0b0000_0010;
    pub const ITEM_DELETED              : u8 = 0b0000_0100;
    pub const ITEM_MARKED               : u8 = 0b0000_1000;
    pub const ITEM_HAS_PARENT_SUB       : u8 = 0b0010_0000;
    pub const ITEM_HAS_RIGHT_ID         : u8 = 0b0100_0000;
    pub const ITEM_HAS_LEFT_ID          : u8 = 0b1000_0000;
    pub const ITEM_HAS_SIBLING          : u8 = 0b1100_0000;
}

#[derive(Debug)]
pub struct ItemFlags(AtomicU8);

impl Default for ItemFlags {
    fn default() -> Self {
        Self(AtomicU8::new(0))
    }
}

impl Clone for ItemFlags {
    fn clone(&self) -> Self {
        Self(AtomicU8::new(self.0.load(Ordering::Acquire)))
    }
}

impl From<u8> for ItemFlags {
    fn from(flags: u8) -> Self {
        Self(AtomicU8::new(flags))
    }
}

#[allow(dead_code)]
impl ItemFlags {
    #[inline(always)]
    pub fn set(&self, flag: u8) {
        self.0.fetch_or(flag, Ordering::SeqCst);
    }

    #[inline(always)]
    pub fn clear(&self, flag: u8) {
        self.0.fetch_and(flag, Ordering::SeqCst);
    }

    #[inline(always)]
    pub fn check(&self, flag: u8) -> bool {
        self.0.load(Ordering::Acquire) & flag == flag
    }

    #[inline(always)]
    pub fn not(&self, flag: u8) -> bool {
        self.0.load(Ordering::Acquire) & flag == 0
    }

    #[inline(always)]
    pub fn keep(&self) -> bool {
        self.check(item_flags::ITEM_KEEP)
    }

    #[inline(always)]
    pub fn set_keep(&self) {
        self.set(item_flags::ITEM_KEEP);
    }

    #[inline(always)]
    pub fn clear_keep(&self) {
        self.clear(item_flags::ITEM_KEEP);
    }

    #[inline(always)]
    pub fn countable(&self) -> bool {
        self.check(item_flags::ITEM_COUNTABLE)
    }

    #[inline(always)]
    pub fn set_countable(&self) {
        self.set(item_flags::ITEM_COUNTABLE);
    }

    #[inline(always)]
    pub fn clear_countable(&self) {
        self.clear(item_flags::ITEM_COUNTABLE);
    }

    #[inline(always)]
    pub fn deleted(&self) -> bool {
        self.check(item_flags::ITEM_DELETED)
    }

    #[inline(always)]
    pub fn set_deleted(&self) {
        self.set(item_flags::ITEM_DELETED);
    }

    #[inline(always)]
    pub fn clear_deleted(&self) {
        self.clear(item_flags::ITEM_DELETED);
    }
}

#[derive(Clone)]
#[cfg_attr(all(test, not(loom)), derive(proptest_derive::Arbitrary))]
pub(crate) struct Item {
    pub id: Id,
    pub origin_left_id: Option<Id>,
    pub origin_right_id: Option<Id>,
    #[cfg_attr(all(test, not(loom)), proptest(value = "None"))]
    pub left: Option<Node>,
    #[cfg_attr(all(test, not(loom)), proptest(value = "None"))]
    pub right: Option<Node>,
    pub parent: Option<Parent>,
    pub parent_sub: Option<String>,
    // make content Arc, so we can share the content between items
    // and item can be readonly and cloned fast.
    // TODO: considering using Cow
    pub content: Arc<Content>,
    #[cfg_attr(all(test, not(loom)), proptest(value = "ItemFlags::default()"))]
    pub flags: ItemFlags,
}

// make all Item readonly
pub(crate) type ItemRef = Somr<Item>;

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl std::fmt::Debug for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Item")
            .field("id", &self.id)
            .field("origin_left_id", &self.origin_left_id)
            .field("origin_right_id", &self.origin_right_id)
            .field("left_id", &self.left.as_ref().map(|i| i.id()))
            .field("right_id", &self.right.as_ref().map(|i| i.id()))
            .field(
                "parent",
                &self.parent.as_ref().map(|p| match p {
                    Parent::Type(_) => "[Type]".to_string(),
                    Parent::String(name) => format!("Parent({name})"),
                    Parent::Id(id) => format!("({}, {})", id.client, id.clock),
                }),
            )
            .field("parent_sub", &self.parent_sub)
            .field("content", &self.content)
            .field("flags", &self.flags)
            .finish()
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Item{}: [{:?}]", self.id, self.content)
    }
}

impl Default for Item {
    fn default() -> Self {
        Self {
            id: Id::default(),
            origin_left_id: None,
            origin_right_id: None,
            left: None,
            right: None,
            parent: None,
            parent_sub: None,
            content: Arc::new(Content::Deleted(0)),
            flags: ItemFlags::from(0),
        }
    }
}

impl Item {
    pub fn new(
        id: Id,
        content: Content,
        left: Somr<Item>,
        right: Somr<Item>,
        parent: Option<Parent>,
        parent_sub: Option<String>,
    ) -> Self {
        let flags = ItemFlags::from(if content.countable() {
            item_flags::ITEM_COUNTABLE
        } else {
            0
        });

        Self {
            id,
            origin_left_id: left.get().map(|left| left.last_id()),
            left: if left.is_some() { Some(Node::Item(left)) } else { None },
            origin_right_id: right.get().map(|right| right.id),
            right: if right.is_some() { Some(Node::Item(right)) } else { None },
            parent,
            parent_sub,
            content: Arc::new(content),
            flags,
        }
    }

    pub fn len(&self) -> u64 {
        self.content.clock_len()
    }

    pub fn deleted(&self) -> bool {
        self.flags.deleted()
    }

    pub fn delete(&self) -> bool {
        if self.deleted() {
            return false;
        }

        // self.content.delete();

        self.flags.set_deleted();

        true
    }

    pub fn countable(&self) -> bool {
        self.flags.countable()
    }

    pub fn indexable(&self) -> bool {
        self.countable() && !self.deleted()
    }

    pub fn last_id(&self) -> Id {
        let Id { client, clock } = self.id;

        Id::new(client, clock + self.len() - 1)
    }

    #[allow(dead_code)]
    #[cfg(any(debug, test))]
    pub(crate) fn print_left(&self) {
        let mut ret = vec![format!("Self{}: [{:?}]", self.id, self.content)];
        let mut left = self.left.clone();

        while let Some(n) = left {
            left = n.left();
            if n.deleted() {
                continue;
            }
            match &n {
                Node::Item(item) => {
                    ret.push(format!("{item}"));
                }
                Node::GC(item) => {
                    ret.push(format!("GC{}: {}", item.id, item.len));
                    break;
                }
                Node::Skip(item) => {
                    ret.push(format!("Skip{}: {}", item.id, item.len));
                    break;
                }
            }
        }
        ret.reverse();

        println!("{}", ret.join(" <- "));
    }

    #[allow(dead_code)]
    #[cfg(any(debug, test))]
    pub(crate) fn print_right(&self) {
        let mut ret = vec![format!("Self{}: [{:?}]", self.id, self.content)];
        let mut right = self.right.clone();

        while let Some(n) = right {
            right = n.right();
            if n.deleted() {
                continue;
            }
            match &n {
                Node::Item(item) => {
                    ret.push(format!("{item}"));
                }
                Node::GC(item) => {
                    ret.push(format!("GC{}: {}", item.id, item.len));
                    break;
                }
                Node::Skip(item) => {
                    ret.push(format!("Skip{}: {}", item.id, item.len));
                    break;
                }
            }
        }

        println!("{}", ret.join(" -> "));
    }
}

impl Item {
    pub(crate) fn read<R: CrdtReader>(decoder: &mut R, id: Id, info: u8, first_5_bit: u8) -> JwstCodecResult<Self> {
        let flags: ItemFlags = info.into();
        let has_left_id = flags.check(item_flags::ITEM_HAS_LEFT_ID);
        let has_right_id = flags.check(item_flags::ITEM_HAS_RIGHT_ID);
        let has_parent_sub = flags.check(item_flags::ITEM_HAS_PARENT_SUB);
        let has_not_sibling = flags.not(item_flags::ITEM_HAS_SIBLING);

        // NOTE: read order must keep the same as the order in yjs
        // TODO: this data structure design will break the cpu OOE, need to be optimized
        let item = Self {
            id,
            origin_left_id: if has_left_id {
                Some(decoder.read_item_id()?)
            } else {
                None
            },
            origin_right_id: if has_right_id {
                Some(decoder.read_item_id()?)
            } else {
                None
            },
            parent: {
                if has_not_sibling {
                    let has_parent = decoder.read_var_u64()? == 1;
                    Some(if has_parent {
                        Parent::String(decoder.read_var_string()?)
                    } else {
                        Parent::Id(decoder.read_item_id()?)
                    })
                } else {
                    None
                }
            },
            parent_sub: if has_not_sibling && has_parent_sub {
                Some(decoder.read_var_string()?)
            } else {
                None
            },
            content: {
                // tag must not GC or Skip, this must process in parse_struct
                debug_assert_ne!(first_5_bit, 0);
                debug_assert_ne!(first_5_bit, 10);
                Arc::new(Content::read(decoder, first_5_bit)?)
            },
            left: None,
            right: None,
            flags: ItemFlags::from(0),
        };

        if item.content.countable() {
            item.flags.set_countable();
        }

        if matches!(item.content.as_ref(), Content::Deleted(_)) {
            item.flags.set_deleted();
        }

        debug_assert!(item.is_valid());

        Ok(item)
    }

    fn get_info(&self) -> u8 {
        let mut info = self.content.get_info();

        if self.origin_left_id.is_some() {
            info |= item_flags::ITEM_HAS_LEFT_ID;
        }
        if self.origin_right_id.is_some() {
            info |= item_flags::ITEM_HAS_RIGHT_ID;
        }
        if self.parent_sub.is_some() {
            info |= item_flags::ITEM_HAS_PARENT_SUB;
        }

        info
    }

    pub(crate) fn is_valid(&self) -> bool {
        let has_id = self.origin_left_id.is_some() || self.origin_right_id.is_some();
        !has_id && self.parent.is_some() || has_id && self.parent.is_none() && self.parent_sub.is_none()
    }

    pub(crate) fn write<W: CrdtWriter>(&self, encoder: &mut W) -> JwstCodecResult {
        let info = self.get_info();
        let has_not_sibling = info & item_flags::ITEM_HAS_SIBLING == 0;

        encoder.write_info(info)?;

        if let Some(left_id) = self.origin_left_id {
            encoder.write_item_id(&left_id)?;
        }
        if let Some(right_id) = self.origin_right_id {
            encoder.write_item_id(&right_id)?;
        }

        if has_not_sibling {
            if let Some(parent) = &self.parent {
                match parent {
                    Parent::String(s) => {
                        encoder.write_var_u64(1)?;
                        encoder.write_var_string(s)?;
                    }
                    Parent::Id(id) => {
                        encoder.write_var_u64(0)?;
                        encoder.write_item_id(id)?;
                    }
                    Parent::Type(ty) => {
                        if let Some(ty) = ty.get() {
                            let ty = ty.read().unwrap();
                            if let Some(item) = ty.item.get() {
                                encoder.write_var_u64(0)?;
                                encoder.write_item_id(&item.id)?;
                            } else if let Some(name) = &ty.root_name {
                                encoder.write_var_u64(1)?;
                                encoder.write_var_string(name)?;
                            }
                        }
                    }
                }
            } else {
                return Err(JwstCodecError::ParentNotFound);
            }

            if let Some(parent_sub) = &self.parent_sub {
                encoder.write_var_string(parent_sub)?;
            }
        }

        self.content.write(encoder)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use proptest::{collection::vec, prelude::*};

    use super::*;

    fn item_round_trip(item: &mut Item) -> JwstCodecResult {
        if !item.is_valid() {
            return Ok(());
        }

        if item.content.countable() {
            item.flags.set_countable();
        }

        let mut encoder = RawEncoder::default();
        item.write(&mut encoder)?;

        let mut decoder = RawDecoder::new(encoder.into_inner());

        let info = decoder.read_info()?;
        let first_5_bit = info & 0b11111;
        let decoded_item = Item::read(&mut decoder, item.id, info, first_5_bit)?;

        assert_eq!(item, &decoded_item);

        Ok(())
    }

    #[cfg(not(loom))]
    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_random_content(mut items in vec(any::<Item>(), 0..10)) {
            for item in &mut items {
                item_round_trip(item).unwrap();
            }
        }
    }
}