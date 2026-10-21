(ns behave.fuel-consumption-test
  "SIGFuelConsumption through the WASM (IDL, glue, enums and units end to
  end) against FOFEM 6.9.4's golden master: one row of
  public/csv/fuel_consumption.csv per plot (copied from behave-lib's
  test/csv, where testFuelConsumption runs the same rows natively)."
  (:require [behave.lib.enums            :as enums]
            [behave.lib.fuel-consumption :as fc]
            [cljs.test                   :refer [is deftest testing]]
            [csv-parser.interface        :refer [parse-csv]])
  (:require-macros [behave.macros :refer [inline-resource]]))

(defn- number
  "parse-csv leaves a negative number as a string."
  [row k]
  (js/parseFloat (get row k)))

(defn within-fofem-tolerance?
  "FOFEM writes floats with 9 digits: 1e-4 relative, as testFofem."
  [expected observed]
  (<= (Math/abs (- expected observed)) (+ (* 1e-4 (Math/abs expected)) 1e-5)))

(def ^:private load-columns
  [["Litter" "Lit"] ["OneHour" "DW1"] ["TenHour" "DW10"] ["HundredHour" "DW100"]
   ["SoundThreeToSixInch" "Snd3"] ["SoundSixToNineInch" "Snd6"] ["SoundNineToTwentyInch" "Snd9"]
   ["SoundTwentyInchAndLarger" "Snd20"] ["RottenThreeToSixInch" "Rot3"] ["RottenSixToNineInch" "Rot6"]
   ["RottenNineToTwentyInch" "Rot9"] ["RottenTwentyInchAndLarger" "Rot20"] ["Duff" "Duf"]
   ["Herbaceous" "Her"] ["Shrub" "Shr"] ["CrownFoliage" "Fol"] ["CrownBranch" "Bra"]])

(def ^:private emission-columns
  [["ParticulateMatterTen" "PM10"] ["ParticulateMatterTwoPointFive" "PM25"] ["Methane" "CH4"]
   ["CarbonMonoxide" "CO"] ["CarbonDioxide" "CO2"] ["NitrogenOxides" "NOX"] ["SulfurDioxide" "SO2"]])

(defn set-inputs!
  "Sets one CSV row's consumption inputs on a SIGFuelConsumption."
  [module row]
  (let [tpa     (enums/loading-units "TonsPerAcre")
        percent (enums/fraction-units "Percent")
        setters {"Litter"                    fc/setFuelLoadLitter
                 "OneHour"                   fc/setFuelLoadOneHour
                 "TenHour"                   fc/setFuelLoadTenHour
                 "HundredHour"               fc/setFuelLoadHundredHour
                 "SoundThreeToSixInch"       fc/setFuelLoadSoundThreeToSixInch
                 "SoundSixToNineInch"        fc/setFuelLoadSoundSixToNineInch
                 "SoundNineToTwentyInch"     fc/setFuelLoadSoundNineToTwentyInch
                 "SoundTwentyInchAndLarger"  fc/setFuelLoadSoundTwentyInchAndLarger
                 "RottenThreeToSixInch"      fc/setFuelLoadRottenThreeToSixInch
                 "RottenSixToNineInch"       fc/setFuelLoadRottenSixToNineInch
                 "RottenNineToTwentyInch"    fc/setFuelLoadRottenNineToTwentyInch
                 "RottenTwentyInchAndLarger" fc/setFuelLoadRottenTwentyInchAndLarger
                 "Duff"                      fc/setFuelLoadDuff
                 "Herbaceous"                fc/setFuelLoadHerbaceous
                 "Shrub"                     fc/setFuelLoadShrub
                 "CrownFoliage"              fc/setFuelLoadCrownFoliage
                 "CrownBranch"               fc/setFuelLoadCrownBranch}]
    (fc/setRegion module (enums/fuel-consumption-region (get row "region")))
    (fc/setCoverGroup module (enums/cover-group (get row "coverGroup")))
    (fc/setBurnSeason module (enums/burn-season (get row "burnSeason")))
    (fc/setFuelCategory module (enums/fuel-category (get row "fuelCategory")))
    (fc/setDuffMoistureMethod module (enums/duff-moisture-method (get row "duffMoistureMethod")))
    (fc/setDuffType module (enums/duff-type (get row "duffType")))
    (fc/setEmissionFactorGroups module
                                (number row "flamingEmissionFactorGroup")
                                (number row "coarseWoodyEmissionFactorGroup")
                                (number row "duffEmissionFactorGroup"))
    (doseq [[component setter] setters]
      (setter module (number row (str "fuelLoad" component)) tpa))
    (fc/setDuffDepth module (number row "duffDepth") (enums/length-units "Inches"))
    (fc/setPercentOfCrownBurned module (number row "percentOfCrownBurned") percent)
    (fc/setMoistureDuff module (number row "moistureDuff") percent)
    (fc/setMoistureTenHour module (number row "moistureTenHour") percent)
    (fc/setMoistureThousandHour module (number row "moistureThousandHour") percent)
    (fc/setMoistureLitter module (number row "moistureLitter") percent)))

(defn observations
  "FOFEM's golden column -> what SIGFuelConsumption reports for it."
  [module]
  (let [tpa (enums/loading-units "TonsPerAcre")
        lba (enums/loading-units "PoundsPerAcre")]
    (concat
     (mapcat (fn [[component prefix]]
               (let [c (enums/fuel-consumption-component component)]
                 [[(str prefix "Con") (fc/getFuelLoadConsumed module c tpa)]
                  [(str prefix "Pos") (fc/getFuelLoadPostfire module c tpa)]]))
             load-columns)
     [["MSE" (fc/getMineralSoilExposed module (enums/fraction-units "Percent"))]
      ["DufDepCon" (fc/getDuffDepthReduction module (enums/length-units "Inches"))]
      ["DufDepPos" (fc/getDuffDepthPostfire module (enums/length-units "Inches"))]]
     (mapcat (fn [[species prefix]]
               (let [s (enums/emission-species species)]
                 [[(str prefix "F") (fc/getFlamingEmission module s lba)]
                  [(str prefix "S") (fc/getSmolderingEmission module s lba)]]))
             emission-columns)
     [["FlaDur" (fc/getFlamingDuration module (enums/time-units "Seconds"))]
      ["SmoDur" (fc/getSmolderingDuration module (enums/time-units "Seconds"))]
      ["FlaCon" (fc/getFuelLoadConsumedFlaming module tpa)]
      ["SmoCon" (fc/getFuelLoadConsumedSmoldering module tpa)]])))

(deftest fuel-consumption-test
  (let [rows (parse-csv (inline-resource "public/csv/fuel_consumption.csv"))]
    (is (= 35 (count rows)) "every golden plot read")
    (doseq [row rows]
      (let [module (fc/init)]
        (set-inputs! module row)
        (testing (get row "plot")
          (is (fc/doFuelConsumptionRun module) (fc/getErrorMessage module))
          (doseq [[column observed] (observations module)]
            (let [expected (number row column)]
              (is (within-fofem-tolerance? expected observed)
                  (str column " expected " expected " observed " observed)))))
        (js/Module.destroy module)))))
