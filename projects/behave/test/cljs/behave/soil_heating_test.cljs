(ns behave.soil-heating-test
  "SIGSoilHeating, fed by SIGFuelConsumption, through the WASM against
  FOFEM 6.9.4's soil runs: one row of public/csv/soil_heating.csv per plot
  (copied from behave-lib's test/csv, where testSoilHeating runs the same
  rows natively)."
  (:require [behave.fuel-consumption-test :refer [set-inputs! within-fofem-tolerance?]]
            [behave.lib.enums             :as enums]
            [behave.lib.fuel-consumption  :as fc]
            [behave.lib.soil-heating      :as soil]
            [cljs.test                    :refer [is deftest testing]]
            [csv-parser.interface         :refer [parse-csv]])
  (:require-macros [behave.macros :refer [inline-resource]]))

(deftest soil-heating-test
  (let [rows (parse-csv (inline-resource "public/csv/soil_heating.csv"))]
    (is (= 20 (count rows)) "every golden soil plot read")
    (doseq [row rows]
      (let [consumption (fc/init)
            module      (soil/init)
            celsius     (enums/temperature-units "Celsius")]
        (set-inputs! consumption row)
        (fc/doFuelConsumptionRun consumption)
        (soil/setSoilFamily module (enums/soil-family (get row "soilFamily")))
        (soil/setSoilMoisture module (js/parseFloat (get row "soilMoisture")) (enums/fraction-units "Percent"))
        (soil/setInputsFromFuelConsumption module consumption)
        (testing (get row "plot")
          (is (soil/doSoilHeatingRun module) (soil/getMessage module))
          (doseq [[column observed] [["Lay0" (soil/getMaximumTemperatureAtLayer module 0 celsius)]
                                     ["Lay2" (soil/getMaximumTemperatureAtLayer module 2 celsius)]
                                     ["Lay4" (soil/getMaximumTemperatureAtLayer module 4 celsius)]
                                     ["Lay6" (soil/getMaximumTemperatureAtLayer module 6 celsius)]
                                     ["Lay60d" (soil/getDeepestLayerReachingSixtyDegreesCelsius module)]
                                     ["Lay275d" (soil/getDeepestLayerReachingTwoHundredSeventyFiveDegreesCelsius module)]]]
            (let [expected (js/parseFloat (get row column))]
              (is (within-fofem-tolerance? expected observed)
                  (str column " expected " expected " observed " observed)))))
        (js/Module.destroy consumption)
        (js/Module.destroy module)))))
