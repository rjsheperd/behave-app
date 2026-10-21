(ns behave.lib.fuel-consumption)

;; Generated from behave.idl's interface SIGFuelConsumption (as hatchet writes these)
(defn init []
  (js/Module.SIGFuelConsumption.))

(defn initializeMembers [self]
  (.initializeMembers self))

(defn distributeThousandHourFuelLoad [self totalFuelLoad loadingUnits rottenFraction fractionUnits distribution]
  (.distributeThousandHourFuelLoad self totalFuelLoad loadingUnits rottenFraction fractionUnits distribution))

(defn doFuelConsumptionRun [self]
  (.doFuelConsumptionRun self))

(defn getBurnSeason [self]
  (.getBurnSeason self))

(defn getCarbonConsumed [self component loadingUnits]
  (.getCarbonConsumed self component loadingUnits))

(defn getCarbonConsumedCrownBranch [self loadingUnits]
  (.getCarbonConsumedCrownBranch self loadingUnits))

(defn getCarbonConsumedCrownFoliage [self loadingUnits]
  (.getCarbonConsumedCrownFoliage self loadingUnits))

(defn getCarbonConsumedDuff [self loadingUnits]
  (.getCarbonConsumedDuff self loadingUnits))

(defn getCarbonConsumedHerbaceous [self loadingUnits]
  (.getCarbonConsumedHerbaceous self loadingUnits))

(defn getCarbonConsumedHundredHour [self loadingUnits]
  (.getCarbonConsumedHundredHour self loadingUnits))

(defn getCarbonConsumedLitter [self loadingUnits]
  (.getCarbonConsumedLitter self loadingUnits))

(defn getCarbonConsumedOneHour [self loadingUnits]
  (.getCarbonConsumedOneHour self loadingUnits))

(defn getCarbonConsumedRottenNineToTwentyInch [self loadingUnits]
  (.getCarbonConsumedRottenNineToTwentyInch self loadingUnits))

(defn getCarbonConsumedRottenSixToNineInch [self loadingUnits]
  (.getCarbonConsumedRottenSixToNineInch self loadingUnits))

(defn getCarbonConsumedRottenThousandHour [self loadingUnits]
  (.getCarbonConsumedRottenThousandHour self loadingUnits))

(defn getCarbonConsumedRottenThreeToSixInch [self loadingUnits]
  (.getCarbonConsumedRottenThreeToSixInch self loadingUnits))

(defn getCarbonConsumedRottenTwentyInchAndLarger [self loadingUnits]
  (.getCarbonConsumedRottenTwentyInchAndLarger self loadingUnits))

(defn getCarbonConsumedShrub [self loadingUnits]
  (.getCarbonConsumedShrub self loadingUnits))

(defn getCarbonConsumedSoundNineToTwentyInch [self loadingUnits]
  (.getCarbonConsumedSoundNineToTwentyInch self loadingUnits))

(defn getCarbonConsumedSoundSixToNineInch [self loadingUnits]
  (.getCarbonConsumedSoundSixToNineInch self loadingUnits))

(defn getCarbonConsumedSoundThousandHour [self loadingUnits]
  (.getCarbonConsumedSoundThousandHour self loadingUnits))

(defn getCarbonConsumedSoundThreeToSixInch [self loadingUnits]
  (.getCarbonConsumedSoundThreeToSixInch self loadingUnits))

(defn getCarbonConsumedSoundTwentyInchAndLarger [self loadingUnits]
  (.getCarbonConsumedSoundTwentyInchAndLarger self loadingUnits))

(defn getCarbonConsumedTenHour [self loadingUnits]
  (.getCarbonConsumedTenHour self loadingUnits))

(defn getCarbonConsumedTotal [self loadingUnits]
  (.getCarbonConsumedTotal self loadingUnits))

(defn getCarbonPostfire [self component loadingUnits]
  (.getCarbonPostfire self component loadingUnits))

(defn getCarbonPostfireCrownBranch [self loadingUnits]
  (.getCarbonPostfireCrownBranch self loadingUnits))

(defn getCarbonPostfireCrownFoliage [self loadingUnits]
  (.getCarbonPostfireCrownFoliage self loadingUnits))

(defn getCarbonPostfireDuff [self loadingUnits]
  (.getCarbonPostfireDuff self loadingUnits))

(defn getCarbonPostfireHerbaceous [self loadingUnits]
  (.getCarbonPostfireHerbaceous self loadingUnits))

(defn getCarbonPostfireHundredHour [self loadingUnits]
  (.getCarbonPostfireHundredHour self loadingUnits))

(defn getCarbonPostfireLitter [self loadingUnits]
  (.getCarbonPostfireLitter self loadingUnits))

(defn getCarbonPostfireOneHour [self loadingUnits]
  (.getCarbonPostfireOneHour self loadingUnits))

(defn getCarbonPostfireRottenNineToTwentyInch [self loadingUnits]
  (.getCarbonPostfireRottenNineToTwentyInch self loadingUnits))

(defn getCarbonPostfireRottenSixToNineInch [self loadingUnits]
  (.getCarbonPostfireRottenSixToNineInch self loadingUnits))

(defn getCarbonPostfireRottenThousandHour [self loadingUnits]
  (.getCarbonPostfireRottenThousandHour self loadingUnits))

(defn getCarbonPostfireRottenThreeToSixInch [self loadingUnits]
  (.getCarbonPostfireRottenThreeToSixInch self loadingUnits))

(defn getCarbonPostfireRottenTwentyInchAndLarger [self loadingUnits]
  (.getCarbonPostfireRottenTwentyInchAndLarger self loadingUnits))

(defn getCarbonPostfireShrub [self loadingUnits]
  (.getCarbonPostfireShrub self loadingUnits))

(defn getCarbonPostfireSoundNineToTwentyInch [self loadingUnits]
  (.getCarbonPostfireSoundNineToTwentyInch self loadingUnits))

(defn getCarbonPostfireSoundSixToNineInch [self loadingUnits]
  (.getCarbonPostfireSoundSixToNineInch self loadingUnits))

(defn getCarbonPostfireSoundThousandHour [self loadingUnits]
  (.getCarbonPostfireSoundThousandHour self loadingUnits))

(defn getCarbonPostfireSoundThreeToSixInch [self loadingUnits]
  (.getCarbonPostfireSoundThreeToSixInch self loadingUnits))

(defn getCarbonPostfireSoundTwentyInchAndLarger [self loadingUnits]
  (.getCarbonPostfireSoundTwentyInchAndLarger self loadingUnits))

(defn getCarbonPostfireTenHour [self loadingUnits]
  (.getCarbonPostfireTenHour self loadingUnits))

(defn getCarbonPostfireTotal [self loadingUnits]
  (.getCarbonPostfireTotal self loadingUnits))

(defn getCarbonPrefire [self component loadingUnits]
  (.getCarbonPrefire self component loadingUnits))

(defn getCarbonPrefireCrownBranch [self loadingUnits]
  (.getCarbonPrefireCrownBranch self loadingUnits))

(defn getCarbonPrefireCrownFoliage [self loadingUnits]
  (.getCarbonPrefireCrownFoliage self loadingUnits))

(defn getCarbonPrefireDuff [self loadingUnits]
  (.getCarbonPrefireDuff self loadingUnits))

(defn getCarbonPrefireHerbaceous [self loadingUnits]
  (.getCarbonPrefireHerbaceous self loadingUnits))

(defn getCarbonPrefireHundredHour [self loadingUnits]
  (.getCarbonPrefireHundredHour self loadingUnits))

(defn getCarbonPrefireLitter [self loadingUnits]
  (.getCarbonPrefireLitter self loadingUnits))

(defn getCarbonPrefireOneHour [self loadingUnits]
  (.getCarbonPrefireOneHour self loadingUnits))

(defn getCarbonPrefireRottenNineToTwentyInch [self loadingUnits]
  (.getCarbonPrefireRottenNineToTwentyInch self loadingUnits))

(defn getCarbonPrefireRottenSixToNineInch [self loadingUnits]
  (.getCarbonPrefireRottenSixToNineInch self loadingUnits))

(defn getCarbonPrefireRottenThousandHour [self loadingUnits]
  (.getCarbonPrefireRottenThousandHour self loadingUnits))

(defn getCarbonPrefireRottenThreeToSixInch [self loadingUnits]
  (.getCarbonPrefireRottenThreeToSixInch self loadingUnits))

(defn getCarbonPrefireRottenTwentyInchAndLarger [self loadingUnits]
  (.getCarbonPrefireRottenTwentyInchAndLarger self loadingUnits))

(defn getCarbonPrefireShrub [self loadingUnits]
  (.getCarbonPrefireShrub self loadingUnits))

(defn getCarbonPrefireSoundNineToTwentyInch [self loadingUnits]
  (.getCarbonPrefireSoundNineToTwentyInch self loadingUnits))

(defn getCarbonPrefireSoundSixToNineInch [self loadingUnits]
  (.getCarbonPrefireSoundSixToNineInch self loadingUnits))

(defn getCarbonPrefireSoundThousandHour [self loadingUnits]
  (.getCarbonPrefireSoundThousandHour self loadingUnits))

(defn getCarbonPrefireSoundThreeToSixInch [self loadingUnits]
  (.getCarbonPrefireSoundThreeToSixInch self loadingUnits))

(defn getCarbonPrefireSoundTwentyInchAndLarger [self loadingUnits]
  (.getCarbonPrefireSoundTwentyInchAndLarger self loadingUnits))

(defn getCarbonPrefireTenHour [self loadingUnits]
  (.getCarbonPrefireTenHour self loadingUnits))

(defn getCarbonPrefireTotal [self loadingUnits]
  (.getCarbonPrefireTotal self loadingUnits))

(defn getCoarseWoodyEmissionFactorGroup [self]
  (.getCoarseWoodyEmissionFactorGroup self))

(defn getConsumptionEquationNumber [self component]
  (.getConsumptionEquationNumber self component))

(defn getConsumptionEquationNumberCrownBranch [self]
  (.getConsumptionEquationNumberCrownBranch self))

(defn getConsumptionEquationNumberCrownFoliage [self]
  (.getConsumptionEquationNumberCrownFoliage self))

(defn getConsumptionEquationNumberDuff [self]
  (.getConsumptionEquationNumberDuff self))

(defn getConsumptionEquationNumberHerbaceous [self]
  (.getConsumptionEquationNumberHerbaceous self))

(defn getConsumptionEquationNumberHundredHour [self]
  (.getConsumptionEquationNumberHundredHour self))

(defn getConsumptionEquationNumberLitter [self]
  (.getConsumptionEquationNumberLitter self))

(defn getConsumptionEquationNumberOneHour [self]
  (.getConsumptionEquationNumberOneHour self))

(defn getConsumptionEquationNumberRottenNineToTwentyInch [self]
  (.getConsumptionEquationNumberRottenNineToTwentyInch self))

(defn getConsumptionEquationNumberRottenSixToNineInch [self]
  (.getConsumptionEquationNumberRottenSixToNineInch self))

(defn getConsumptionEquationNumberRottenThousandHour [self]
  (.getConsumptionEquationNumberRottenThousandHour self))

(defn getConsumptionEquationNumberRottenThreeToSixInch [self]
  (.getConsumptionEquationNumberRottenThreeToSixInch self))

(defn getConsumptionEquationNumberRottenTwentyInchAndLarger [self]
  (.getConsumptionEquationNumberRottenTwentyInchAndLarger self))

(defn getConsumptionEquationNumberShrub [self]
  (.getConsumptionEquationNumberShrub self))

(defn getConsumptionEquationNumberSoundNineToTwentyInch [self]
  (.getConsumptionEquationNumberSoundNineToTwentyInch self))

(defn getConsumptionEquationNumberSoundSixToNineInch [self]
  (.getConsumptionEquationNumberSoundSixToNineInch self))

(defn getConsumptionEquationNumberSoundThousandHour [self]
  (.getConsumptionEquationNumberSoundThousandHour self))

(defn getConsumptionEquationNumberSoundThreeToSixInch [self]
  (.getConsumptionEquationNumberSoundThreeToSixInch self))

(defn getConsumptionEquationNumberSoundTwentyInchAndLarger [self]
  (.getConsumptionEquationNumberSoundTwentyInchAndLarger self))

(defn getConsumptionEquationNumberTenHour [self]
  (.getConsumptionEquationNumberTenHour self))

(defn getCoverGroup [self]
  (.getCoverGroup self))

(defn getDuffBurnTime [self timeUnits]
  (.getDuffBurnTime self timeUnits))

(defn getDuffConsumptionEquation [self]
  (.getDuffConsumptionEquation self))

(defn getDuffDepth [self lengthUnits]
  (.getDuffDepth self lengthUnits))

(defn getDuffDepthPostfire [self lengthUnits]
  (.getDuffDepthPostfire self lengthUnits))

(defn getDuffDepthPrefire [self lengthUnits]
  (.getDuffDepthPrefire self lengthUnits))

(defn getDuffDepthReduction [self lengthUnits]
  (.getDuffDepthReduction self lengthUnits))

(defn getDuffDepthReductionEquation [self]
  (.getDuffDepthReductionEquation self))

(defn getDuffDepthReductionEquationNumber [self]
  (.getDuffDepthReductionEquationNumber self))

(defn getDuffEmissionFactorGroup [self]
  (.getDuffEmissionFactorGroup self))

(defn getDuffFireIntensity [self intensityUnits]
  (.getDuffFireIntensity self intensityUnits))

(defn getDuffMoistureMethod [self]
  (.getDuffMoistureMethod self))

(defn getDuffSmolderingEmission [self species loadingUnits]
  (.getDuffSmolderingEmission self species loadingUnits))

(defn getDuffType [self]
  (.getDuffType self))

(defn getEmissionsDuffSmolderingCarbonDioxide [self loadingUnits]
  (.getEmissionsDuffSmolderingCarbonDioxide self loadingUnits))

(defn getEmissionsDuffSmolderingCarbonMonoxide [self loadingUnits]
  (.getEmissionsDuffSmolderingCarbonMonoxide self loadingUnits))

(defn getEmissionsDuffSmolderingMethane [self loadingUnits]
  (.getEmissionsDuffSmolderingMethane self loadingUnits))

(defn getEmissionsDuffSmolderingNitrogenOxides [self loadingUnits]
  (.getEmissionsDuffSmolderingNitrogenOxides self loadingUnits))

(defn getEmissionsDuffSmolderingParticulateMatterTen [self loadingUnits]
  (.getEmissionsDuffSmolderingParticulateMatterTen self loadingUnits))

(defn getEmissionsDuffSmolderingParticulateMatterTwoPointFive [self loadingUnits]
  (.getEmissionsDuffSmolderingParticulateMatterTwoPointFive self loadingUnits))

(defn getEmissionsDuffSmolderingSulfurDioxide [self loadingUnits]
  (.getEmissionsDuffSmolderingSulfurDioxide self loadingUnits))

(defn getEmissionsFlamingCarbonDioxide [self loadingUnits]
  (.getEmissionsFlamingCarbonDioxide self loadingUnits))

(defn getEmissionsFlamingCarbonMonoxide [self loadingUnits]
  (.getEmissionsFlamingCarbonMonoxide self loadingUnits))

(defn getEmissionsFlamingMethane [self loadingUnits]
  (.getEmissionsFlamingMethane self loadingUnits))

(defn getEmissionsFlamingNitrogenOxides [self loadingUnits]
  (.getEmissionsFlamingNitrogenOxides self loadingUnits))

(defn getEmissionsFlamingParticulateMatterTen [self loadingUnits]
  (.getEmissionsFlamingParticulateMatterTen self loadingUnits))

(defn getEmissionsFlamingParticulateMatterTwoPointFive [self loadingUnits]
  (.getEmissionsFlamingParticulateMatterTwoPointFive self loadingUnits))

(defn getEmissionsFlamingSulfurDioxide [self loadingUnits]
  (.getEmissionsFlamingSulfurDioxide self loadingUnits))

(defn getEmissionsSmolderingCarbonDioxide [self loadingUnits]
  (.getEmissionsSmolderingCarbonDioxide self loadingUnits))

(defn getEmissionsSmolderingCarbonMonoxide [self loadingUnits]
  (.getEmissionsSmolderingCarbonMonoxide self loadingUnits))

(defn getEmissionsSmolderingMethane [self loadingUnits]
  (.getEmissionsSmolderingMethane self loadingUnits))

(defn getEmissionsSmolderingNitrogenOxides [self loadingUnits]
  (.getEmissionsSmolderingNitrogenOxides self loadingUnits))

(defn getEmissionsSmolderingParticulateMatterTen [self loadingUnits]
  (.getEmissionsSmolderingParticulateMatterTen self loadingUnits))

(defn getEmissionsSmolderingParticulateMatterTwoPointFive [self loadingUnits]
  (.getEmissionsSmolderingParticulateMatterTwoPointFive self loadingUnits))

(defn getEmissionsSmolderingSulfurDioxide [self loadingUnits]
  (.getEmissionsSmolderingSulfurDioxide self loadingUnits))

(defn getErrorMessage [self]
  (.getErrorMessage self))

(defn getFlamingDuration [self timeUnits]
  (.getFlamingDuration self timeUnits))

(defn getFlamingEmission [self species loadingUnits]
  (.getFlamingEmission self species loadingUnits))

(defn getFlamingEmissionFactorGroup [self]
  (.getFlamingEmissionFactorGroup self))

(defn getFuelCategory [self]
  (.getFuelCategory self))

(defn getFuelLoadConsumed [self component loadingUnits]
  (.getFuelLoadConsumed self component loadingUnits))

(defn getFuelLoadConsumedCrownBranch [self loadingUnits]
  (.getFuelLoadConsumedCrownBranch self loadingUnits))

(defn getFuelLoadConsumedCrownFoliage [self loadingUnits]
  (.getFuelLoadConsumedCrownFoliage self loadingUnits))

(defn getFuelLoadConsumedDuff [self loadingUnits]
  (.getFuelLoadConsumedDuff self loadingUnits))

(defn getFuelLoadConsumedFlaming [self loadingUnits]
  (.getFuelLoadConsumedFlaming self loadingUnits))

(defn getFuelLoadConsumedHerbaceous [self loadingUnits]
  (.getFuelLoadConsumedHerbaceous self loadingUnits))

(defn getFuelLoadConsumedHundredHour [self loadingUnits]
  (.getFuelLoadConsumedHundredHour self loadingUnits))

(defn getFuelLoadConsumedLitter [self loadingUnits]
  (.getFuelLoadConsumedLitter self loadingUnits))

(defn getFuelLoadConsumedOneHour [self loadingUnits]
  (.getFuelLoadConsumedOneHour self loadingUnits))

(defn getFuelLoadConsumedRottenNineToTwentyInch [self loadingUnits]
  (.getFuelLoadConsumedRottenNineToTwentyInch self loadingUnits))

(defn getFuelLoadConsumedRottenSixToNineInch [self loadingUnits]
  (.getFuelLoadConsumedRottenSixToNineInch self loadingUnits))

(defn getFuelLoadConsumedRottenThousandHour [self loadingUnits]
  (.getFuelLoadConsumedRottenThousandHour self loadingUnits))

(defn getFuelLoadConsumedRottenThreeToSixInch [self loadingUnits]
  (.getFuelLoadConsumedRottenThreeToSixInch self loadingUnits))

(defn getFuelLoadConsumedRottenTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadConsumedRottenTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadConsumedShrub [self loadingUnits]
  (.getFuelLoadConsumedShrub self loadingUnits))

(defn getFuelLoadConsumedSmoldering [self loadingUnits]
  (.getFuelLoadConsumedSmoldering self loadingUnits))

(defn getFuelLoadConsumedSoundNineToTwentyInch [self loadingUnits]
  (.getFuelLoadConsumedSoundNineToTwentyInch self loadingUnits))

(defn getFuelLoadConsumedSoundSixToNineInch [self loadingUnits]
  (.getFuelLoadConsumedSoundSixToNineInch self loadingUnits))

(defn getFuelLoadConsumedSoundThousandHour [self loadingUnits]
  (.getFuelLoadConsumedSoundThousandHour self loadingUnits))

(defn getFuelLoadConsumedSoundThreeToSixInch [self loadingUnits]
  (.getFuelLoadConsumedSoundThreeToSixInch self loadingUnits))

(defn getFuelLoadConsumedSoundTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadConsumedSoundTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadConsumedTenHour [self loadingUnits]
  (.getFuelLoadConsumedTenHour self loadingUnits))

(defn getFuelLoadConsumedTotal [self loadingUnits]
  (.getFuelLoadConsumedTotal self loadingUnits))

(defn getFuelLoadCrownBranch [self loadingUnits]
  (.getFuelLoadCrownBranch self loadingUnits))

(defn getFuelLoadCrownFoliage [self loadingUnits]
  (.getFuelLoadCrownFoliage self loadingUnits))

(defn getFuelLoadDuff [self loadingUnits]
  (.getFuelLoadDuff self loadingUnits))

(defn getFuelLoadHerbaceous [self loadingUnits]
  (.getFuelLoadHerbaceous self loadingUnits))

(defn getFuelLoadHundredHour [self loadingUnits]
  (.getFuelLoadHundredHour self loadingUnits))

(defn getFuelLoadLitter [self loadingUnits]
  (.getFuelLoadLitter self loadingUnits))

(defn getFuelLoadOneHour [self loadingUnits]
  (.getFuelLoadOneHour self loadingUnits))

(defn getFuelLoadPostfire [self component loadingUnits]
  (.getFuelLoadPostfire self component loadingUnits))

(defn getFuelLoadPostfireCrownBranch [self loadingUnits]
  (.getFuelLoadPostfireCrownBranch self loadingUnits))

(defn getFuelLoadPostfireCrownFoliage [self loadingUnits]
  (.getFuelLoadPostfireCrownFoliage self loadingUnits))

(defn getFuelLoadPostfireDuff [self loadingUnits]
  (.getFuelLoadPostfireDuff self loadingUnits))

(defn getFuelLoadPostfireHerbaceous [self loadingUnits]
  (.getFuelLoadPostfireHerbaceous self loadingUnits))

(defn getFuelLoadPostfireHundredHour [self loadingUnits]
  (.getFuelLoadPostfireHundredHour self loadingUnits))

(defn getFuelLoadPostfireLitter [self loadingUnits]
  (.getFuelLoadPostfireLitter self loadingUnits))

(defn getFuelLoadPostfireOneHour [self loadingUnits]
  (.getFuelLoadPostfireOneHour self loadingUnits))

(defn getFuelLoadPostfireRottenNineToTwentyInch [self loadingUnits]
  (.getFuelLoadPostfireRottenNineToTwentyInch self loadingUnits))

(defn getFuelLoadPostfireRottenSixToNineInch [self loadingUnits]
  (.getFuelLoadPostfireRottenSixToNineInch self loadingUnits))

(defn getFuelLoadPostfireRottenThousandHour [self loadingUnits]
  (.getFuelLoadPostfireRottenThousandHour self loadingUnits))

(defn getFuelLoadPostfireRottenThreeToSixInch [self loadingUnits]
  (.getFuelLoadPostfireRottenThreeToSixInch self loadingUnits))

(defn getFuelLoadPostfireRottenTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadPostfireRottenTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadPostfireShrub [self loadingUnits]
  (.getFuelLoadPostfireShrub self loadingUnits))

(defn getFuelLoadPostfireSoundNineToTwentyInch [self loadingUnits]
  (.getFuelLoadPostfireSoundNineToTwentyInch self loadingUnits))

(defn getFuelLoadPostfireSoundSixToNineInch [self loadingUnits]
  (.getFuelLoadPostfireSoundSixToNineInch self loadingUnits))

(defn getFuelLoadPostfireSoundThousandHour [self loadingUnits]
  (.getFuelLoadPostfireSoundThousandHour self loadingUnits))

(defn getFuelLoadPostfireSoundThreeToSixInch [self loadingUnits]
  (.getFuelLoadPostfireSoundThreeToSixInch self loadingUnits))

(defn getFuelLoadPostfireSoundTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadPostfireSoundTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadPostfireTenHour [self loadingUnits]
  (.getFuelLoadPostfireTenHour self loadingUnits))

(defn getFuelLoadPostfireTotal [self loadingUnits]
  (.getFuelLoadPostfireTotal self loadingUnits))

(defn getFuelLoadPrefire [self component loadingUnits]
  (.getFuelLoadPrefire self component loadingUnits))

(defn getFuelLoadPrefireCrownBranch [self loadingUnits]
  (.getFuelLoadPrefireCrownBranch self loadingUnits))

(defn getFuelLoadPrefireCrownFoliage [self loadingUnits]
  (.getFuelLoadPrefireCrownFoliage self loadingUnits))

(defn getFuelLoadPrefireDuff [self loadingUnits]
  (.getFuelLoadPrefireDuff self loadingUnits))

(defn getFuelLoadPrefireHerbaceous [self loadingUnits]
  (.getFuelLoadPrefireHerbaceous self loadingUnits))

(defn getFuelLoadPrefireHundredHour [self loadingUnits]
  (.getFuelLoadPrefireHundredHour self loadingUnits))

(defn getFuelLoadPrefireLitter [self loadingUnits]
  (.getFuelLoadPrefireLitter self loadingUnits))

(defn getFuelLoadPrefireOneHour [self loadingUnits]
  (.getFuelLoadPrefireOneHour self loadingUnits))

(defn getFuelLoadPrefireRottenNineToTwentyInch [self loadingUnits]
  (.getFuelLoadPrefireRottenNineToTwentyInch self loadingUnits))

(defn getFuelLoadPrefireRottenSixToNineInch [self loadingUnits]
  (.getFuelLoadPrefireRottenSixToNineInch self loadingUnits))

(defn getFuelLoadPrefireRottenThousandHour [self loadingUnits]
  (.getFuelLoadPrefireRottenThousandHour self loadingUnits))

(defn getFuelLoadPrefireRottenThreeToSixInch [self loadingUnits]
  (.getFuelLoadPrefireRottenThreeToSixInch self loadingUnits))

(defn getFuelLoadPrefireRottenTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadPrefireRottenTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadPrefireShrub [self loadingUnits]
  (.getFuelLoadPrefireShrub self loadingUnits))

(defn getFuelLoadPrefireSoundNineToTwentyInch [self loadingUnits]
  (.getFuelLoadPrefireSoundNineToTwentyInch self loadingUnits))

(defn getFuelLoadPrefireSoundSixToNineInch [self loadingUnits]
  (.getFuelLoadPrefireSoundSixToNineInch self loadingUnits))

(defn getFuelLoadPrefireSoundThousandHour [self loadingUnits]
  (.getFuelLoadPrefireSoundThousandHour self loadingUnits))

(defn getFuelLoadPrefireSoundThreeToSixInch [self loadingUnits]
  (.getFuelLoadPrefireSoundThreeToSixInch self loadingUnits))

(defn getFuelLoadPrefireSoundTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadPrefireSoundTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadPrefireTenHour [self loadingUnits]
  (.getFuelLoadPrefireTenHour self loadingUnits))

(defn getFuelLoadPrefireTotal [self loadingUnits]
  (.getFuelLoadPrefireTotal self loadingUnits))

(defn getFuelLoadRottenNineToTwentyInch [self loadingUnits]
  (.getFuelLoadRottenNineToTwentyInch self loadingUnits))

(defn getFuelLoadRottenSixToNineInch [self loadingUnits]
  (.getFuelLoadRottenSixToNineInch self loadingUnits))

(defn getFuelLoadRottenThreeToSixInch [self loadingUnits]
  (.getFuelLoadRottenThreeToSixInch self loadingUnits))

(defn getFuelLoadRottenTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadRottenTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadShrub [self loadingUnits]
  (.getFuelLoadShrub self loadingUnits))

(defn getFuelLoadSoundNineToTwentyInch [self loadingUnits]
  (.getFuelLoadSoundNineToTwentyInch self loadingUnits))

(defn getFuelLoadSoundSixToNineInch [self loadingUnits]
  (.getFuelLoadSoundSixToNineInch self loadingUnits))

(defn getFuelLoadSoundThreeToSixInch [self loadingUnits]
  (.getFuelLoadSoundThreeToSixInch self loadingUnits))

(defn getFuelLoadSoundTwentyInchAndLarger [self loadingUnits]
  (.getFuelLoadSoundTwentyInchAndLarger self loadingUnits))

(defn getFuelLoadTenHour [self loadingUnits]
  (.getFuelLoadTenHour self loadingUnits))

(defn getFuelLoadThousandHour [self loadingUnits]
  (.getFuelLoadThousandHour self loadingUnits))

(defn getHerbaceousConsumptionEquation [self]
  (.getHerbaceousConsumptionEquation self))

(defn getLitterConsumptionEquation [self]
  (.getLitterConsumptionEquation self))

(defn getMineralSoilExposed [self fractionUnits]
  (.getMineralSoilExposed self fractionUnits))

(defn getMineralSoilExposureEquation [self]
  (.getMineralSoilExposureEquation self))

(defn getMineralSoilExposureEquationNumber [self]
  (.getMineralSoilExposureEquationNumber self))

(defn getMoistureDuff [self moistureUnits]
  (.getMoistureDuff self moistureUnits))

(defn getMoistureLitter [self moistureUnits]
  (.getMoistureLitter self moistureUnits))

(defn getMoistureTenHour [self moistureUnits]
  (.getMoistureTenHour self moistureUnits))

(defn getMoistureThousandHour [self moistureUnits]
  (.getMoistureThousandHour self moistureUnits))

(defn getPercentConsumed [self component fractionUnits]
  (.getPercentConsumed self component fractionUnits))

(defn getPercentConsumedCrownBranch [self fractionUnits]
  (.getPercentConsumedCrownBranch self fractionUnits))

(defn getPercentConsumedCrownFoliage [self fractionUnits]
  (.getPercentConsumedCrownFoliage self fractionUnits))

(defn getPercentConsumedDuff [self fractionUnits]
  (.getPercentConsumedDuff self fractionUnits))

(defn getPercentConsumedHerbaceous [self fractionUnits]
  (.getPercentConsumedHerbaceous self fractionUnits))

(defn getPercentConsumedHundredHour [self fractionUnits]
  (.getPercentConsumedHundredHour self fractionUnits))

(defn getPercentConsumedLitter [self fractionUnits]
  (.getPercentConsumedLitter self fractionUnits))

(defn getPercentConsumedOneHour [self fractionUnits]
  (.getPercentConsumedOneHour self fractionUnits))

(defn getPercentConsumedRottenNineToTwentyInch [self fractionUnits]
  (.getPercentConsumedRottenNineToTwentyInch self fractionUnits))

(defn getPercentConsumedRottenSixToNineInch [self fractionUnits]
  (.getPercentConsumedRottenSixToNineInch self fractionUnits))

(defn getPercentConsumedRottenThousandHour [self fractionUnits]
  (.getPercentConsumedRottenThousandHour self fractionUnits))

(defn getPercentConsumedRottenThreeToSixInch [self fractionUnits]
  (.getPercentConsumedRottenThreeToSixInch self fractionUnits))

(defn getPercentConsumedRottenTwentyInchAndLarger [self fractionUnits]
  (.getPercentConsumedRottenTwentyInchAndLarger self fractionUnits))

(defn getPercentConsumedShrub [self fractionUnits]
  (.getPercentConsumedShrub self fractionUnits))

(defn getPercentConsumedSoundNineToTwentyInch [self fractionUnits]
  (.getPercentConsumedSoundNineToTwentyInch self fractionUnits))

(defn getPercentConsumedSoundSixToNineInch [self fractionUnits]
  (.getPercentConsumedSoundSixToNineInch self fractionUnits))

(defn getPercentConsumedSoundThousandHour [self fractionUnits]
  (.getPercentConsumedSoundThousandHour self fractionUnits))

(defn getPercentConsumedSoundThreeToSixInch [self fractionUnits]
  (.getPercentConsumedSoundThreeToSixInch self fractionUnits))

(defn getPercentConsumedSoundTwentyInchAndLarger [self fractionUnits]
  (.getPercentConsumedSoundTwentyInchAndLarger self fractionUnits))

(defn getPercentConsumedTenHour [self fractionUnits]
  (.getPercentConsumedTenHour self fractionUnits))

(defn getPercentConsumedTotal [self fractionUnits]
  (.getPercentConsumedTotal self fractionUnits))

(defn getPercentDuffDepthReduction [self fractionUnits]
  (.getPercentDuffDepthReduction self fractionUnits))

(defn getPercentOfCrownBurned [self fractionUnits]
  (.getPercentOfCrownBurned self fractionUnits))

(defn getRegion [self]
  (.getRegion self))

(defn getShrubConsumptionEquation [self]
  (.getShrubConsumptionEquation self))

(defn getSmolderingDuration [self timeUnits]
  (.getSmolderingDuration self timeUnits))

(defn getSmolderingEmission [self species loadingUnits]
  (.getSmolderingEmission self species loadingUnits))

(defn getUseConsumptionEquations [self]
  (.getUseConsumptionEquations self))

(defn getUseLegacyEmissionFactors [self]
  (.getUseLegacyEmissionFactors self))

(defn isFuelbedIgnited [self]
  (.isFuelbedIgnited self))

(defn setBurnSeason [self burnSeason]
  (.setBurnSeason self burnSeason))

(defn setCoverGroup [self coverGroup]
  (.setCoverGroup self coverGroup))

(defn setDuffConsumptionEquation [self equation]
  (.setDuffConsumptionEquation self equation))

(defn setDuffDepth [self duffDepth lengthUnits]
  (.setDuffDepth self duffDepth lengthUnits))

(defn setDuffDepthReductionEquation [self equation]
  (.setDuffDepthReductionEquation self equation))

(defn setDuffMoistureMethod [self duffMoistureMethod]
  (.setDuffMoistureMethod self duffMoistureMethod))

(defn setDuffType [self duffType]
  (.setDuffType self duffType))

(defn setEmissionFactorGroups [self flamingGroupNumber coarseWoodyGroupNumber duffGroupNumber]
  (.setEmissionFactorGroups self flamingGroupNumber coarseWoodyGroupNumber duffGroupNumber))

(defn setFuelCategory [self fuelCategory]
  (.setFuelCategory self fuelCategory))

(defn setFuelLoadCrownBranch [self fuelLoad loadingUnits]
  (.setFuelLoadCrownBranch self fuelLoad loadingUnits))

(defn setFuelLoadCrownFoliage [self fuelLoad loadingUnits]
  (.setFuelLoadCrownFoliage self fuelLoad loadingUnits))

(defn setFuelLoadDuff [self fuelLoad loadingUnits]
  (.setFuelLoadDuff self fuelLoad loadingUnits))

(defn setFuelLoadHerbaceous [self fuelLoad loadingUnits]
  (.setFuelLoadHerbaceous self fuelLoad loadingUnits))

(defn setFuelLoadHundredHour [self fuelLoad loadingUnits]
  (.setFuelLoadHundredHour self fuelLoad loadingUnits))

(defn setFuelLoadLitter [self fuelLoad loadingUnits]
  (.setFuelLoadLitter self fuelLoad loadingUnits))

(defn setFuelLoadOneHour [self fuelLoad loadingUnits]
  (.setFuelLoadOneHour self fuelLoad loadingUnits))

(defn setFuelLoadRottenNineToTwentyInch [self fuelLoad loadingUnits]
  (.setFuelLoadRottenNineToTwentyInch self fuelLoad loadingUnits))

(defn setFuelLoadRottenSixToNineInch [self fuelLoad loadingUnits]
  (.setFuelLoadRottenSixToNineInch self fuelLoad loadingUnits))

(defn setFuelLoadRottenThreeToSixInch [self fuelLoad loadingUnits]
  (.setFuelLoadRottenThreeToSixInch self fuelLoad loadingUnits))

(defn setFuelLoadRottenTwentyInchAndLarger [self fuelLoad loadingUnits]
  (.setFuelLoadRottenTwentyInchAndLarger self fuelLoad loadingUnits))

(defn setFuelLoadShrub [self fuelLoad loadingUnits]
  (.setFuelLoadShrub self fuelLoad loadingUnits))

(defn setFuelLoadSoundNineToTwentyInch [self fuelLoad loadingUnits]
  (.setFuelLoadSoundNineToTwentyInch self fuelLoad loadingUnits))

(defn setFuelLoadSoundSixToNineInch [self fuelLoad loadingUnits]
  (.setFuelLoadSoundSixToNineInch self fuelLoad loadingUnits))

(defn setFuelLoadSoundThreeToSixInch [self fuelLoad loadingUnits]
  (.setFuelLoadSoundThreeToSixInch self fuelLoad loadingUnits))

(defn setFuelLoadSoundTwentyInchAndLarger [self fuelLoad loadingUnits]
  (.setFuelLoadSoundTwentyInchAndLarger self fuelLoad loadingUnits))

(defn setFuelLoadTenHour [self fuelLoad loadingUnits]
  (.setFuelLoadTenHour self fuelLoad loadingUnits))

(defn setHerbaceousConsumptionEquation [self equation]
  (.setHerbaceousConsumptionEquation self equation))

(defn setLitterConsumptionEquation [self equation]
  (.setLitterConsumptionEquation self equation))

(defn setMineralSoilExposureEquation [self equation]
  (.setMineralSoilExposureEquation self equation))

(defn setMoistureDuff [self moisture moistureUnits]
  (.setMoistureDuff self moisture moistureUnits))

(defn setMoistureLitter [self moisture moistureUnits]
  (.setMoistureLitter self moisture moistureUnits))

(defn setMoistureTenHour [self moisture moistureUnits]
  (.setMoistureTenHour self moisture moistureUnits))

(defn setMoistureThousandHour [self moisture moistureUnits]
  (.setMoistureThousandHour self moisture moistureUnits))

(defn setPercentOfCrownBurned [self percentOfCrownBurned fractionUnits]
  (.setPercentOfCrownBurned self percentOfCrownBurned fractionUnits))

(defn setRegion [self region]
  (.setRegion self region))

(defn setShrubConsumptionEquation [self equation]
  (.setShrubConsumptionEquation self equation))

(defn setUseConsumptionEquations [self useConsumptionEquations]
  (.setUseConsumptionEquations self useConsumptionEquations))

(defn setUseLegacyEmissionFactors [self useLegacyEmissionFactors]
  (.setUseLegacyEmissionFactors self useLegacyEmissionFactors))
