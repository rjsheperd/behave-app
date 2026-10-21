(ns behave.lib.soil-heating)

;; Generated from behave.idl's interface SIGSoilHeating (as hatchet writes these)
(defn init []
  (js/Module.SIGSoilHeating.))

(defn initializeMembers [self]
  (.initializeMembers self))

(defn addFireIntensity [self woodLitterIntensity herbShrubIntensity intensityUnits]
  (.addFireIntensity self woodLitterIntensity herbShrubIntensity intensityUnits))

(defn clearFireIntensities [self]
  (.clearFireIntensities self))

(defn doSoilHeatingRun [self]
  (.doSoilHeatingRun self))

(defn getDeepestLayerReachingSixtyDegreesCelsius [self]
  (.getDeepestLayerReachingSixtyDegreesCelsius self))

(defn getDeepestLayerReachingTwoHundredSeventyFiveDegreesCelsius [self]
  (.getDeepestLayerReachingTwoHundredSeventyFiveDegreesCelsius self))

(defn getDuffConsumed [self fractionUnits]
  (.getDuffConsumed self fractionUnits))

(defn getDuffDepthPostfire [self lengthUnits]
  (.getDuffDepthPostfire self lengthUnits))

(defn getDuffDepthPrefire [self lengthUnits]
  (.getDuffDepthPrefire self lengthUnits))

(defn getDuffHeatEfficiency [self fractionUnits]
  (.getDuffHeatEfficiency self fractionUnits))

(defn getDuffLoadPrefire [self loadingUnits]
  (.getDuffLoadPrefire self loadingUnits))

(defn getDuffMoisture [self moistureUnits]
  (.getDuffMoisture self moistureUnits))

(defn getFuelbedIgnited [self]
  (.getFuelbedIgnited self))

(defn getHerbShrubFireIntensity [self index intensityUnits]
  (.getHerbShrubFireIntensity self index intensityUnits))

(defn getHerbShrubHeatEfficiency [self fractionUnits]
  (.getHerbShrubHeatEfficiency self fractionUnits))

(defn getMaximumTemperatureAtLayer [self layer temperatureUnits]
  (.getMaximumTemperatureAtLayer self layer temperatureUnits))

(defn getMessage [self]
  (.getMessage self))

(defn getNumberOfFireIntensities [self]
  (.getNumberOfFireIntensities self))

(defn getNumberOfTimeSteps [self]
  (.getNumberOfTimeSteps self))

(defn getProportionOfHeatReachingSoilThroughDuff [self fractionUnits]
  (.getProportionOfHeatReachingSoilThroughDuff self fractionUnits))

(defn getSoilFamily [self]
  (.getSoilFamily self))

(defn getSoilHeatingModelUsed [self]
  (.getSoilHeatingModelUsed self))

(defn getSoilMoisture [self moistureUnits]
  (.getSoilMoisture self moistureUnits))

(defn getStartingSoilTemperature [self temperatureUnits]
  (.getStartingSoilTemperature self temperatureUnits))

(defn getTemperatureAtLayerAndTimeStep [self layer timeStep temperatureUnits]
  (.getTemperatureAtLayerAndTimeStep self layer timeStep temperatureUnits))

(defn getTimeOfMaximumTemperatureAtLayer [self layer timeUnits]
  (.getTimeOfMaximumTemperatureAtLayer self layer timeUnits))

(defn getTimeStep [self timeUnits]
  (.getTimeStep self timeUnits))

(defn getWoodLitterFireIntensity [self index intensityUnits]
  (.getWoodLitterFireIntensity self index intensityUnits))

(defn getWoodLitterHeatEfficiency [self fractionUnits]
  (.getWoodLitterHeatEfficiency self fractionUnits))

(defn setDuffConsumed [self duffConsumed fractionUnits]
  (.setDuffConsumed self duffConsumed fractionUnits))

(defn setDuffDepthPostfire [self duffDepth lengthUnits]
  (.setDuffDepthPostfire self duffDepth lengthUnits))

(defn setDuffDepthPrefire [self duffDepth lengthUnits]
  (.setDuffDepthPrefire self duffDepth lengthUnits))

(defn setDuffHeatEfficiency [self efficiency fractionUnits]
  (.setDuffHeatEfficiency self efficiency fractionUnits))

(defn setDuffLoadPrefire [self duffLoad loadingUnits]
  (.setDuffLoadPrefire self duffLoad loadingUnits))

(defn setDuffMoisture [self duffMoisture moistureUnits]
  (.setDuffMoisture self duffMoisture moistureUnits))

(defn setFuelbedIgnited [self isFuelbedIgnited]
  (.setFuelbedIgnited self isFuelbedIgnited))

(defn setHerbShrubHeatEfficiency [self efficiency fractionUnits]
  (.setHerbShrubHeatEfficiency self efficiency fractionUnits))

(defn setInputsFromFuelConsumption [self fuelConsumption]
  (.setInputsFromFuelConsumption self fuelConsumption))

(defn setSoilFamily [self soilFamily]
  (.setSoilFamily self soilFamily))

(defn setSoilMoisture [self soilMoisture moistureUnits]
  (.setSoilMoisture self soilMoisture moistureUnits))

(defn setStartingSoilTemperature [self temperature temperatureUnits]
  (.setStartingSoilTemperature self temperature temperatureUnits))

(defn setWoodLitterHeatEfficiency [self efficiency fractionUnits]
  (.setWoodLitterHeatEfficiency self efficiency fractionUnits))
