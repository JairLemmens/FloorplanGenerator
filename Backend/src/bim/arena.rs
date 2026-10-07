use crate::bim::base_types::{Id};

struct Slot<T> {generation: u32,value: Option<T>}
pub struct Arena<T> {slots: Vec<Slot<T>>,free: Vec<u32>}

impl<T> Arena<T> {
    pub fn new() -> Self {Self {slots: Vec::new(),free: Vec::new()}}

    pub fn insert(&mut self, value: T) -> Id {
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            slot.value = Some(value);
            Id {index, generation: slot.generation}
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot {generation: 0, value: Some(value)});
            Id {index,generation: 0}
        }
    }

    pub fn get(&self, id: Id) -> Option<&T> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {return None}
        slot.value.as_ref()
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {return None}
        slot.value.as_mut()
    }

    pub fn remove(&mut self, id: Id) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        slot.generation = slot.generation.checked_add(1).expect("arena generation overflow");
        self.free.push(id.index);
        Some(value)
    }
    
    pub fn iter(&self) -> impl Iterator<Item = &T> {self.slots.iter().filter_map(|slot| slot.value.as_ref())}

    pub fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|_| Id {
                index: index as u32,
                generation: slot.generation,
            })
        })
    }
    pub fn iter_with_ids(&self) -> impl Iterator<Item = (Id, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(index, slot)| {
            slot.value.as_ref().map(|value| {
                (
                    Id {
                        index: index as u32,
                        generation: slot.generation,
                    },
                    value,
                )
            })
        })
    }
}