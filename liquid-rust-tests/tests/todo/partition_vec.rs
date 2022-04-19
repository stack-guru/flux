//! A [disjoint-sets/union-find] implementation of a vector partitioned in sets.
//!
//! See [`PartitionVec<T>`] for more information.
//!
//! [disjoint-sets/union-find]: https://en.wikipedia.org/wiki/Disjoint-set_data_structure
//! [`PartitionVec<T>`]: struct.PartitionVec.html

#![allow(unused_attributes)]
#![feature(register_tool)]
#![register_tool(lr)]

#[path = "../lib/rvec.rs"]
pub mod rvec;
use rvec::RVec;

use {
    std::{
        cmp::Ordering,
    },
};

/// Inline definition from metadata.rs
#[derive(Copy, Clone, PartialEq)]
#[lr::opaque]
#[lr::refined_by(parent: int)]
pub struct Metadata {
    /// The parent of the value in its sets tree.
    /// These form an upside down tree where each child has the index of its parent.
    parent: usize,
}

impl Metadata {
    /// Create a new `Metadata` for an element with the given index.
    //#[trusted]
    //#[ensures(result.parent() == index && result.rank() == 0 && result.link == index)]
    #[lr::assume]
    #[lr::ty(fn() -> Metadata @ 0)]
    pub(crate) fn new(index: usize) -> Self {
        Self {
            parent: index,
            //link: index,
            //rank: 0,
        }
    }

    /// Return the `parent` variable.
    #[lr::assume]
    #[lr::ty(fn<parent: int>(&Metadata@parent) -> usize@parent)]
    pub(crate) fn parent(&self) -> usize {
        self.parent
    }

    /// Set the `parent` variable.
    #[lr::assume]
    #[lr::ty(fn<value: int>(self: &Metadata; ref<self>, usize@value) -> usize; self: Metadata{x: x == value})]
    pub(crate) fn set_parent(&mut self, value: usize) -> usize {
        self.parent = value;
        value
    }

    //#[lr::assume]
    //#[lr::ty(fn(self: &parent1@Metadata, other: &parent2@Metadata) -> bool[parent1 == parent2])]
    pub fn eq(&self, other: &Metadata) -> bool {
        self.parent == other.parent
    }
}

/// A [disjoint-sets/union-find] implementation of a vector partitioned in sets.
///
/// Most methods that are defined on a `Vec` also work on a `PartitionVec`.
/// In addition to this each element stored in the `PartitionVec` is a member of a set.
/// Initially each element has its own set but sets can be joined with the `union` method.
///
/// In addition to the normal implementation we store an additional index for each element.
/// These indices form a circular linked list of the set the element is in.
/// This allows for fast iteration of the set using the `set` method
/// and is used to speed up the performance of other methods.
///
/// This implementation chooses not to expose the `find` method and instead has a `same_set` method.
/// This is so that the representative of the set stays an implementation detail which gives
/// us more freedom to change it behind the scenes for improved performance.
///
/// # Examples
///
/// ```
/// # #[macro_use]
/// # extern crate partitions;
/// #
/// # fn main() {
/// let mut partition_vec = partition_vec!['a', 'b', 'c', 'd'];
/// partition_vec.union(1, 2);
/// partition_vec.union(2, 3);
///
/// assert!(partition_vec.same_set(1, 3));
///
/// for (index, &value) in partition_vec.set(1) {
///     assert!(index >= 1);
///     assert!(index <= 3);
///     assert!(value != 'a');
/// }
/// # }
/// ```
///
/// [disjoint-sets/union-find]: https://en.wikipedia.org/wiki/Disjoint-set_data_structure
#[lr::refined_by(size: int)]
pub struct PartitionVec {
    /// Each index has a value.
    /// We store these in a separate `Vec` so we can easily dereference it to a slice.
    //#[lr::field(RVec<i32>@size)]
    //data: RVec<i32>,
    /// The metadata for each value, this `Vec` will always have the same size as `values`.
    #[lr::field(RVec<Metadata>@size)]
    meta: RVec<Metadata>,
}

/// Creates a [`PartitionVec`] containing the arguments.
///
/// There are tree forms of the `partition_vec!` macro:
///
/// - Create a [`PartitionVec`] containing a given list of elements all in distinct sets:
///
/// ```
/// # #[macro_use]
/// # extern crate partitions;
/// #
/// # fn main() {
/// let partition_vec = partition_vec!['a', 'b', 'c'];
///
/// assert!(partition_vec[0] == 'a');
/// assert!(partition_vec[1] == 'b');
/// assert!(partition_vec[2] == 'c');
///
/// assert!(partition_vec.is_singleton(0));
/// assert!(partition_vec.is_singleton(1));
/// assert!(partition_vec.is_singleton(2));
/// # }
/// ```
///
/// - Create a [`PartitionVec`] containing a given list of elements in the sets specified:
///
/// ```
/// # #[macro_use]
/// # extern crate partitions;
/// #
/// # fn main() {
/// let partition_vec = partition_vec![
///     'a' => 0,
///     'b' => 1,
///     'c' => 2,
///     'd' => 1,
///     'e' => 0,
/// ];
///
/// assert!(partition_vec[0] == 'a');
/// assert!(partition_vec[1] == 'b');
/// assert!(partition_vec[2] == 'c');
/// assert!(partition_vec[3] == 'd');
/// assert!(partition_vec[4] == 'e');
///
/// assert!(partition_vec.same_set(0, 4));
/// assert!(partition_vec.same_set(1, 3));
/// assert!(partition_vec.is_singleton(2));
/// # }
/// ```
///
/// You can use any identifiers that implement `Hash` and `Eq`.
/// Elements with the same set identifiers will be placed in the same set.
/// These identifiers will only be used when constructing a [`PartitionVec`]
/// and will not be stored further.
/// This means `println!("{:?}", partition_vec![3 => 'a', 1 => 'a'])` will display `[3 => 0, 1 => 0]`.
///
/// - Create a [`PartitionVec`] of distinct sets from a given element and size:
///
/// ```
/// # #[macro_use]
/// # extern crate partitions;
/// #
/// # fn main() {
/// let partition_vec = partition_vec!['a'; 3];
///
/// assert!(partition_vec[0] == 'a');
/// assert!(partition_vec[1] == 'a');
/// assert!(partition_vec[2] == 'a');
///
/// assert!(partition_vec.is_singleton(0));
/// assert!(partition_vec.is_singleton(1));
/// assert!(partition_vec.is_singleton(2));
/// # }
/// ```
///
/// [`PartitionVec`]: partition_vec/struct.PartitionVec.html
///
/*
#[macro_export]
macro_rules! partition_vec {
    ($elem: expr; $len: expr) => {
        $crate::PartitionVec::from_elem($elem, $len);
    };
    ($($elem: expr),*) => {
        {
            let len = partitions_count_expr![$($elem),*];
            let mut partition_vec = $crate::PartitionVec::with_capacity(len);

            $(
                partition_vec.push($elem);
            )*

            partition_vec
        }
    };
    ($($elem: expr,)*) => {
        partition_vec![$($elem),*];
    };
    ($($elem: expr => $set: expr),*) => {
        {
            let len = partitions_count_expr![$($elem),*];
            let mut partition_vec = $crate::PartitionVec::with_capacity(len);
            let mut map = ::std::collections::HashMap::new();

            $(
                let last_index = partition_vec.len();
                partition_vec.push($elem);

                if let Some(&index) = map.get(&$set) {
                    partition_vec.union(index, last_index);
                } else {
                    map.insert($set, last_index);
                }
            )*

            partition_vec
        }
    };
    ($($elem: expr => $set: expr,)*) => {
        partition_vec![$($elem => $set),*];
    }
}*/

impl PartitionVec {
    /// Constructs a new, empty `PartitionVec<T>`.
    ///
    /// The `PartitionVec<T>` will not allocate until elements are pushed onto it.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use partitions::PartitionVec;
    ///
    /// let mut partition_vec: PartitionVec<()> = PartitionVec::new();
    /// ```
    #[lr::ty(fn() -> PartitionVec@0)]
    pub fn new(size: usize) -> Self {
        let meta = RVec::new();
        Self {
            //data: RVec::new(),
            meta,
        }
    }
    /*#[requires(first_index < self.meta.len())]
    #[requires(second_index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]
    #[ensures(self.data.len() == self.meta.len())]
    #[ensures(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[ensures(self.data.len() == self.meta.len())]*/
    pub fn union(&mut self, first_index: usize, second_index: usize) {
        let i = self.find(first_index);
        let j = self.find(second_index);

        if i == j {
            return
        }

        // We swap the values of the links.
        //let link_i = self.meta.lookup(i).link();
        //let link_j = self.meta.lookup(j).link();
        //self.meta.lookup(i).set_link(link_j);
        //self.meta.lookup(j).set_link(link_i);

        // We add to the tree with the highest rank.
        // match Ord::cmp(&self.meta.lookup(i).rank(), &self.meta.lookup(j).rank()) {
        //     Ordering::Less => {
        //         self.meta.lookup(i).set_parent(j);
        //     },
        //     Ordering::Equal => {
        //         // We add the first tree to the second tree.
        //         self.meta.lookup(i).set_parent(j);
        //         // The second tree becomes larger.
        //         self.meta.lookup(j).set_rank(self.meta.lookup(j).rank() + 1);
        //     },
        //     Ordering::Greater => {
        //         self.meta.lookup(j).set_parent(i);
        //     },
        // }
        self.meta.get_mut(i).set_parent(j);
    }

    #[inline]
    // #[requires(first_index < self.meta.len())]
    // #[requires(second_index < self.meta.len())]
    // #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    // #[requires(self.data.len() == self.meta.len())]
    pub fn same_set(&mut self, first_index: usize, second_index: usize) -> bool {
        self.find(first_index) == self.find(second_index)
    }

    #[inline]
    // #[requires(first_index < self.meta.len())]
    // #[requires(second_index < self.meta.len())]
    // #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    // #[requires(self.data.len() == self.meta.len())]
    pub fn other_sets(&mut self, first_index: usize, second_index: usize) -> bool {
        self.find(first_index) != self.find(second_index)
    }

    /*#[requires(index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]
    #[trusted]*/
    // pub fn make_singleton(&mut self, index: usize) {
    //     let mut current = self.meta.lookup(index).link();

    //     if current != index {
    //         // We make this the new root.
    //         let root = current;
    //         //self.meta.lookup(root).set_rank(1);

    //         // Change to use local variable as workaround based on
    //         // https://github.com/viperproject/prusti-dev/issues/786
    //         let mut current_meta = self.meta.get_mut(current);

    //         // All parents except for the last are updated.
    //         while current_meta.link() != index {
    //             current_meta.set_parent(root);

    //             current_meta = self.meta.lookup(current_meta.link());
    //         }

    //         // We change the last parent and link.
    //         current_meta.set_parent(root);
    //         current_meta.set_link(root);
    //     }

    //     self.meta.store(index, Metadata::new(index));
    // }

    #[inline]
    /*#[requires(index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]*/
    // pub fn is_singleton(&self, index: usize) -> bool {
    //     self.meta.lookup(index).link() == index
    // }

    /// #[requires(first_index < self.meta.len())]
    /*#[requires(index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]*/
    // pub fn len_of_set(&self, index: usize) -> usize {
    //     let mut current = self.meta.lookup(index).link();
    //     let mut count = 1;

    //     while current != index {
    //         body_invariant!(self.data.len() == old(self.data.len()) && self.meta.len() == old(self.meta.len()));
    //         body_invariant!(current < self.meta.len());

    //         current = self.meta.lookup(current).link();
    //         count += 1;
    //     }

    //     count
    // }

    /*#[requires(index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]
    #[ensures(result < self.meta.len())]*/
    #[lr::ty(fn<size: int{size >= 0}>(self: PartitionVec@size; ref<self>, usize{v: v < size}) -> usize{v: v < size})]
    pub(crate) fn find(&mut self, index: usize) -> usize {
        // If the node is its own parent we have found the root.
        if self.meta.get(index).parent() == index {
            index
        } else {
            // This method is recursive so each parent on the way to the root is updated.
            let parent = self.meta.get(index).parent();
            let root = self.find(parent);

            // We update the parent to the root for a lower tree.
            let mut metadata = self.meta.get_mut(index);
            metadata.set_parent(root);

            root
        }
    }

    #[inline]
    /*#[requires(index < self.meta.len())]
    #[requires(forall(|x: usize| x < self.meta.len() ==> self.meta.lookup(x).parent < self.meta.len() && self.meta.lookup(x).link < self.meta.len()))]
    #[requires(self.data.len() == self.meta.len())]
    #[ensures(result < self.meta.len())]*/
    pub(crate) fn find_final(&self, mut index: usize) -> usize {
        while index != self.meta.get(index).parent() {
            // body_invariant!(self.data.len() == old(self.data.len()) && self.meta.len() == old(self.meta.len()));
            // body_invariant!(index < self.meta.len());

            index = self.meta.get(index).parent();
        }

        index
    }
}

pub fn main() {

}