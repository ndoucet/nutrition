use crate::domain::food::{FoodId, State};
use crate::domain::quantity::Grams;
use crate::Error;

/// A positive number of servings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Servings(u32);

impl Servings {
    /// Rejects zero, negative, and values above `u32::MAX`.
    ///
    /// # Arguments
    ///
    /// * `servings` - number of servings.
    pub fn new(servings: i64) -> Result<Self, Error> {
        u32::try_from(servings)
            .ok()
            .filter(|s| *s > 0)
            .map(Servings)
            .ok_or(Error::InvalidServings { servings })
    }

    /// The number of servings.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// One ingredient of a recipe: a food, its state, and its weight in that state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecipeItem {
    pub food_id: FoodId,
    pub state: State,
    pub quantity: Grams,
}

/// A reusable list of ingredients (D10). Items are identified by position (D17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    name: String,
    servings: Servings,
    cooked_weight: Option<Grams>,
    items: Vec<RecipeItem>,
}

impl Recipe {
    /// An empty recipe with an unknown cooked weight.
    ///
    /// # Arguments
    ///
    /// * `name` - name of the recipe.
    /// * `servings` - number of servings the recipe makes.
    pub fn new(name: impl Into<String>, servings: Servings) -> Self {
        Recipe {
            name: name.into(),
            servings,
            cooked_weight: None,
            items: Vec::new(),
        }
    }

    /// Name of the recipe.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Number of servings the recipe makes.
    pub fn servings(&self) -> Servings {
        self.servings
    }

    /// Replaces the number of servings.
    ///
    /// # Arguments
    ///
    /// * `servings` - new number of servings.
    pub fn set_servings(&mut self, servings: Servings) {
        self.servings = servings;
    }

    /// Total weight of the finished dish, if known.
    pub fn cooked_weight(&self) -> Option<Grams> {
        self.cooked_weight
    }

    /// Sets the weight of the finished dish; `None` marks it unknown.
    ///
    /// # Arguments
    ///
    /// * `cooked_weight` - weight of the finished dish in grams, or `None` when unknown.
    pub fn set_cooked_weight(&mut self, cooked_weight: Option<Grams>) {
        self.cooked_weight = cooked_weight;
    }

    /// Ingredients in recipe order; the index is the position (D17).
    pub fn items(&self) -> &[RecipeItem] {
        &self.items
    }

    /// Appends an ingredient and returns its position.
    ///
    /// # Arguments
    ///
    /// * `food_id` - identifier of the food.
    /// * `state` - preparation state in which the food is weighed.
    /// * `quantity` - weight of the food in that state, in grams.
    pub fn add_ingredient(&mut self, food_id: FoodId, state: State, quantity: Grams) -> usize {
        self.items.push(RecipeItem {
            food_id,
            state,
            quantity,
        });
        self.items.len() - 1
    }

    /// Removes the ingredient at `index`; later ingredients shift down by one.
    ///
    /// # Arguments
    ///
    /// * `index` - position of the ingredient, from 0.
    pub fn remove_ingredient(&mut self, index: usize) -> Result<RecipeItem, Error> {
        if index >= self.items.len() {
            return Err(Error::IngredientIndexOutOfRange {
                index,
                len: self.items.len(),
            });
        }
        Ok(self.items.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Servings accept only positive values.
    #[test]
    fn rejects_zero_and_negative_servings() {
        assert_eq!(
            Servings::new(0),
            Err(Error::InvalidServings { servings: 0 })
        );
        assert_eq!(
            Servings::new(-2),
            Err(Error::InvalidServings { servings: -2 })
        );
    }

    /// Removing an ingredient shifts later positions; an out-of-range position is
    /// an error.
    #[test]
    fn removes_ingredient_by_position() {
        let mut recipe = Recipe::new("r", Servings::new(1).unwrap());
        let g = Grams::new(10).unwrap();
        recipe.add_ingredient(FoodId(1), State::Raw, g);
        recipe.add_ingredient(FoodId(2), State::Raw, g);
        assert_eq!(recipe.remove_ingredient(0).unwrap().food_id, FoodId(1));
        assert_eq!(recipe.items()[0].food_id, FoodId(2));
        assert_eq!(
            recipe.remove_ingredient(1),
            Err(Error::IngredientIndexOutOfRange { index: 1, len: 1 })
        );
    }
}
