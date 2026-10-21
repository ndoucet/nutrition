# Data Sources and Licensing

## 1. Objectives

The application depends on two distinct classes of external data:

- Food composition data.
- Nutritional reference values and calculation methods.

Each source must be documented and its permitted use verified before redistribution.

## 2. Food composition databases

### Initial source: CIQUAL

CIQUAL, the French food composition table published by ANSES, is the only food source for the initial phases (see [DECISIONS.md](DECISIONS.md), D3).

- Its nutrient definitions follow European conventions, which are the application's base.
- It provides French and English food names.
- Its value markers map directly to the application's value states: `traces`, `< x`, and `-`.
- It is published under the French *Licence Ouverte / Etalab 2.0* (to be confirmed against the current terms).
- Its export files use decimal commas.

### Later: USDA FoodData Central

USDA FoodData Central may be added later. Its nutrient definitions differ from the European ones (for example, its carbohydrate usually includes fibre). Before USDA data is imported, a conversion process must be defined that maps each USDA nutrient to a European definition, or creates a separate nutrient when no exact mapping exists.

### Other sources

Other national food composition databases may be considered where their licensing and coverage are suitable.

Source selection should consider:

- Geographic relevance.
- Food coverage.
- Nutrient coverage.
- Raw and cooked coverage.
- Data quality and traceability.
- Update frequency.
- Redistribution and attribution requirements.

## 3. Provenance

For each imported food record, retain as much of the following as practical:

- Source name.
- Source record identifier.
- Source version or retrieval date.
- Original food name.
- Original nutrient values, markers, and units.
- Applied unit conversions.
- Mapping decisions, including the raw/cooked pairing.
- Licensing or attribution information.

Imported records should be reproducible and traceable to their original source.

## 4. Licensing

Before including a dataset in a distributed application:

- Read the current terms of use.
- Check redistribution and derivative-database conditions.
- Preserve required attribution.
- Record any restrictions on commercial use.
- Review applicable requirements for bundled data and exported datasets.

Do not assume that publicly accessible data is unrestricted.

## 5. Import validation

An import pipeline should:

- Parse the source format (including decimal commas for CIQUAL).
- Validate required fields.
- Normalize nutrient identifiers to the application's nutrient codes.
- Convert units to the application's canonical units and integer fixed-point values.
- Map source markers to value states (known, zero, trace, below limit, unknown).
- Pair raw and cooked records of the same food, using a documented mapping file. Foods with a single record use it for both states.
- Record source provenance.
- Detect duplicate or conflicting records.
- Report rejected or incomplete records.

Do not silently merge conflicting nutrient values from multiple sources.

## 6. Reference values

Nutritional recommendations come from EFSA Dietary Reference Values (see [DECISIONS.md](DECISIONS.md), D4). ANSES recommendations are a possible national alternative.

The BMR equations are the Henry (2005) equations, as used by EFSA.

For each reference, preserve:

- Issuing authority.
- Publication or revision.
- Population criteria.
- Nutrient definition and unit.
- Reference type (AR, PRI, AI, RI, UL).
- Applicable calculation method.
- Source URL or bibliographic reference, with page or table number.
- Who verified the value against the source, and when.

A recommendation must not be labelled as universal if it applies only to a particular population.

## 7. Initial dataset

Begin with a small dataset that can be checked manually: 10 to 20 CIQUAL foods, some with both raw and cooked records, and a few recipes with hand-calculated expected results.

Each record should include enough provenance to validate its values. Expand coverage only after the import and validation process is reliable.
