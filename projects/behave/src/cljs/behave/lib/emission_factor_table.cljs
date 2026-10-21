(ns behave.lib.emission-factor-table)

;; Generated from behave.idl's interface EmissionFactorTable (as hatchet writes these)
(defn init []
  (js/Module.EmissionFactorTable.))

(defn getCoverType [self groupNumber]
  (.getCoverType self groupNumber))

(defn getEmissionFactor [self groupNumber species emissionFactorUnits]
  (.getEmissionFactor self groupNumber species emissionFactorUnits))

(defn getEmissionFactorBySpeciesIndex [self groupNumber speciesIndex emissionFactorUnits]
  (.getEmissionFactorBySpeciesIndex self groupNumber speciesIndex emissionFactorUnits))

(defn getGroupType [self groupNumber]
  (.getGroupType self groupNumber))

(defn getNumberOfGroups [self]
  (.getNumberOfGroups self))

(defn getNumberOfSpecies [self]
  (.getNumberOfSpecies self))

(defn getSpeciesCode [self speciesIndex]
  (.getSpeciesCode self speciesIndex))

(defn getSpeciesName [self speciesIndex]
  (.getSpeciesName self speciesIndex))

(defn isValidGroup [self groupNumber]
  (.isValidGroup self groupNumber))
