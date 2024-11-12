#include <iostream>
#include "behaveRun.h"
#include "fuelModels.h"
#include "testUtils.h"

// Define the error tolerance for double values
constexpr double error_tolerance = 1e-06;

// Testing Structs
struct CrownTestInputs {
  int fuelModelNumber = 0.0;
  double moistureOneHour = 0.0;
  double moistureTenHour = 0.0;
  double moistureHundredHour = 0.0;
  double moistureLiveHerbaceous = 0.0;
  double moistureLiveWoody = 0.0;
  double moistureFoliar = 0.0;
  FractionUnits::FractionUnitsEnum moistureUnits = FractionUnits::Percent;
  double windSpeed = 0.0;
  WindHeightInputMode::WindHeightInputModeEnum windHeightInputMode = WindHeightInputMode::TwentyFoot;
  SpeedUnits::SpeedUnitsEnum windSpeedUnits = SpeedUnits::MilesPerHour;
  double windDirection = 0.0;
  WindAndSpreadOrientationMode::WindAndSpreadOrientationModeEnum windAndSpreadOrientationMode = WindAndSpreadOrientationMode::RelativeToNorth;
  double windAdjustmentFactor = 1.0;
  WindAdjustmentFactorCalculationMethod::WindAdjustmentFactorCalculationMethodEnum windAdjustmentFactorCalculationMethod = WindAdjustmentFactorCalculationMethod::UserInput;
  double slope = 0.0;
  SlopeUnits::SlopeUnitsEnum slopeUnits = SlopeUnits::Percent;
  double aspect = 0.0;
  double canopyCover = 0.0;
  FractionUnits::FractionUnitsEnum coverUnits = FractionUnits::Percent;
  double canopyHeight = 0.0;
  LengthUnits::LengthUnitsEnum canopyHeightUnits = LengthUnits::Feet;
  double crownRatio = 0.0;
  double canopyBaseHeight = 0.0;
  double canopyBulkDensity = 0.0;
  DensityUnits::DensityUnitsEnum canopyBulkDensityUnits = DensityUnits::PoundsPerCubicFoot;
};

struct CrownTestOutputs {
  double lengthToWidthRatio = 0.00;
  double fireSpreadRate = 0.00;
  double flameLength = 0.00;
  double firelineIntensity = 0.00;
  int fireType = -1;
};

// Testing Function Headers
void testCrownModule(int row, struct TestInfo& testInfo, CrownTestInputs& crownInputs, CrownTestOutputs& crownOutputs, BehaveRun& behaveRun);

int main(int argc, char * argv[])
{
  if (argc == 1) {
    std::cout << "Unable continue tests. Please supply CSV filename.\n" << std::endl;
    return 1;
  }

  TestInfo testInfo;
  FuelModels fuelModels;
  CrownTestInputs crownInputs;
  CrownTestOutputs crownOutputs;
  SpeciesMasterTable mortalitySpeciesTable;
  BehaveRun behaveRun(fuelModels, mortalitySpeciesTable);

  std::string csvFilename{argv[1]};

  /* Units/Enum Maps */
  std::map<std::string, LengthUnits::LengthUnitsEnum> lengthUnits{
    {"Feet", LengthUnits::Feet},
    {"Inches", LengthUnits::Inches},
    {"Millimeters", LengthUnits::Millimeters},
    {"Centimeters", LengthUnits::Centimeters},
    {"Meters", LengthUnits::Meters},
    {"Chains", LengthUnits::Chains},
    {"Miles", LengthUnits::Miles},
    {"Kilometers", LengthUnits::Kilometers}
  };

  std::map<std::string, SlopeUnits::SlopeUnitsEnum> slopeUnits{
    {"Degrees", SlopeUnits::Degrees},
    {"Percent", SlopeUnits::Percent}};

  std::map<std::string, SpeedUnits::SpeedUnitsEnum> speedUnits{
    {"FeetPerMinute", SpeedUnits::FeetPerMinute},
    {"ChainsPerHour", SpeedUnits::ChainsPerHour},
    {"MetersPerSecond", SpeedUnits::MetersPerSecond},
    {"MetersPerMinute", SpeedUnits::MetersPerMinute},
    {"MetersPerHour", SpeedUnits::MetersPerHour},
    {"MilesPerHour", SpeedUnits::MilesPerHour},
    {"KilometersPerHour", SpeedUnits::KilometersPerHour}
  };

  std::map<std::string, FractionUnits::FractionUnitsEnum> coverUnits{
    {"Fraction", FractionUnits::Fraction},
    {"Percent", FractionUnits::Percent}};

  std::map<std::string, FractionUnits::FractionUnitsEnum> moistureUnits{
    {"Fraction", FractionUnits::Fraction},
    {"Percent", FractionUnits::Percent}};

  std::map<std::string, DensityUnits::DensityUnitsEnum> densityUnits{
    {"PoundsPerCubicFoot", DensityUnits::PoundsPerCubicFoot},
    {"KilogramsPerCubicMeter", DensityUnits::KilogramsPerCubicMeter}};

  std::map<std::string, WindHeightInputMode::WindHeightInputModeEnum> windHeightInputMode{
    {"DirectMidflame", WindHeightInputMode::DirectMidflame},
    {"TwentyFoot", WindHeightInputMode::TwentyFoot},
    {"TenMeter", WindHeightInputMode::TenMeter}};

  std::map<std::string, WindAndSpreadOrientationMode::WindAndSpreadOrientationModeEnum> windAndSpreadOrientationMode{
    {"RelativeToUpslope", WindAndSpreadOrientationMode::RelativeToUpslope},
    {"RelativeToNorth", WindAndSpreadOrientationMode::RelativeToNorth}
  };

  std::map<std::string, WindAdjustmentFactorCalculationMethod::WindAdjustmentFactorCalculationMethodEnum> windAdjustmentFactorCalculationMethod{
    {"UserInput", WindAdjustmentFactorCalculationMethod::UserInput},
    {"UseCrownRatio", WindAdjustmentFactorCalculationMethod::UseCrownRatio},
    {"DontUseCrownRatio", WindAdjustmentFactorCalculationMethod::DontUseCrownRatio}
  };

  std::map<std::string, FireType::FireTypeEnum> fireType{
    {"Surface", FireType::Surface},
    {"Torching", FireType::Torching},
    {"ConditionalCrownFire", FireType::ConditionalCrownFire},
    {"Crowning", FireType::Crowning}
  };


  // CSV Reading
  std::vector<std::string> csvHeaders;
  std::vector<std::map<std::string, std::string>> csvStringRows;
  std::vector<std::map<std::string, double>> csvDoubleRows;

  int result = parseCSVFile(csvFilename, csvHeaders, csvStringRows, csvDoubleRows);

  if (result == 0) {
    printCSVData(csvHeaders, csvStringRows, csvDoubleRows);
  }

  std::cout << "Performing tests with: " << csvStringRows.size() << " samples.\n";

  // Perform Tests using CSV Inputs
  for (int i = 0; i < csvStringRows.size(); i++) {

    std::map<std::string, double> doubleRow = csvDoubleRows[i];
    std::map<std::string, std::string> stringRow = csvStringRows[i];

    // Set up Inputs
    crownInputs.fuelModelNumber = doubleRow["fuelModelNumber"];
    crownInputs.moistureOneHour = doubleRow["moistureOneHour"];
    crownInputs.moistureTenHour = doubleRow["moistureTenHour"];
    crownInputs.moistureHundredHour = doubleRow["moistureHundredHour"];
    crownInputs.moistureLiveHerbaceous = doubleRow["moistureLiveHerbaceous"];
    crownInputs.moistureLiveWoody = doubleRow["moistureLiveWoody"];
    crownInputs.moistureFoliar = doubleRow["moistureFoliar"];
    crownInputs.moistureUnits = moistureUnits[stringRow["moistureUnits"]];
    crownInputs.windSpeed = doubleRow["windSpeed"];
    crownInputs.windSpeedUnits = speedUnits[stringRow["windSpeedUnits"]];
    crownInputs.windHeightInputMode = windHeightInputMode[stringRow["windHeightInputMode"]];
    crownInputs.windDirection = doubleRow["windDirection"];
    crownInputs.windAndSpreadOrientationMode = windAndSpreadOrientationMode[stringRow["windAndSpreadOrientationMode"]];
    crownInputs.windAdjustmentFactor = doubleRow["windAdjFactor"];
    crownInputs.windAdjustmentFactorCalculationMethod = windAdjustmentFactorCalculationMethod[stringRow["windAdjFactorCalcMethod"]];
    crownInputs.slope = doubleRow["slope"];
    crownInputs.slopeUnits = slopeUnits[stringRow["slopeUnits"]];
    crownInputs.aspect = doubleRow["aspect"];
    crownInputs.canopyCover = doubleRow["canopyCover"];
    crownInputs.coverUnits = coverUnits[stringRow["coverUnits"]];
    crownInputs.canopyHeight = doubleRow["canopyHeight"];
    crownInputs.canopyBaseHeight = doubleRow["canopyBaseHeight"];
    crownInputs.canopyHeightUnits = lengthUnits[stringRow["canopyHeightUnits"]];
    crownInputs.crownRatio = doubleRow["crownRatio"];
    crownInputs.canopyBulkDensity = doubleRow["canopyBulkDensity"];
    crownInputs.canopyBulkDensityUnits = densityUnits[stringRow["canopyBulkDensityUnits"]];

    // Set up Outputs
    crownOutputs.lengthToWidthRatio = doubleRow["lengthToWidthRatio"];
    crownOutputs.fireSpreadRate = doubleRow["fireSpreadRate"];
    crownOutputs.flameLength = doubleRow["flameLength"];
    crownOutputs.firelineIntensity = doubleRow["firelineIntensity"];
    crownOutputs.fireType = fireType[stringRow["fireType"]];

    // Run test
    testCrownModule(i, testInfo, crownInputs, crownOutputs, behaveRun);

  };
  std::cout << "Total tests performed: " << testInfo.numTotalTests << "\n";
  std::cout << "Total tests passed: " << testInfo.numPassed << "\n";
  std::cout << "Total tests failed: " << testInfo.numFailed << "\n\n";

  //#ifndef NDEBUG
  //    // Make Visual Studio wait while in debug mode
  //    std::cout << "Press Enter to continue . . .";
  //    std::cin.get();
  //#endif
  //    return 0;

}

void testCrownModule(int row, struct TestInfo& testInfo, CrownTestInputs& inputs, CrownTestOutputs& expected, BehaveRun& behaveRun)
{
  std::cout << "Testing Crown module\n";

  string testName = "";

  double observedLengthToWidthRatio = 0;
  double observedSpreadRate = 0;
  double observedFlameLength = 0;
  double observedFirelineIntensity = 0;
  int observedFireType = (int)FireType::Surface;

  // Set up inputs
  behaveRun.crown.updateCrownInputs(inputs.fuelModelNumber,
                                    inputs.moistureOneHour,
                                    inputs.moistureTenHour,
                                    inputs.moistureHundredHour,
                                    inputs.moistureLiveHerbaceous,
                                    inputs.moistureLiveWoody,
                                    inputs.moistureFoliar,
                                    inputs.moistureUnits,
                                    inputs.windSpeed,
                                    inputs.windSpeedUnits,
                                    inputs.windHeightInputMode,
                                    inputs.windDirection,
                                    inputs.windAndSpreadOrientationMode,
                                    inputs.slope,
                                    inputs.slopeUnits,
                                    inputs.aspect,
                                    inputs.canopyCover,
                                    inputs.coverUnits,
                                    inputs.canopyHeight,
                                    inputs.canopyBaseHeight,
                                    inputs.canopyHeightUnits,
                                    inputs.crownRatio,
                                    FractionUnits::Fraction,
                                    inputs.canopyBulkDensity,
                                    inputs.canopyBulkDensityUnits);

  // Wind Adjustment Factor
  behaveRun.crown.setUserProvidedWindAdjustmentFactor(inputs.windAdjustmentFactor);
  behaveRun.crown.setWindAdjustmentFactorCalculationMethod(inputs.windAdjustmentFactorCalculationMethod);

  // Perform Run
  behaveRun.crown.doCrownRunRothermel();

  // Compare Results
  if (expected.lengthToWidthRatio != 0) {
    testName = "Test length to width ratio";
    observedLengthToWidthRatio = behaveRun.crown.getCrownFireLengthToWidthRatio();
    reportTestResult(row, testInfo, testName, observedLengthToWidthRatio, expected.lengthToWidthRatio, error_tolerance);
  }

  if (expected.fireSpreadRate != 0) {
    testName = "Test fire spread rate";
    observedSpreadRate = behaveRun.crown.getFinalSpreadRate(SpeedUnits::ChainsPerHour);
    reportTestResult(row, testInfo, testName, observedSpreadRate, expected.fireSpreadRate, error_tolerance);
  }

  if (expected.flameLength != 0) {
    testName = "Test flame length";
    observedFlameLength = behaveRun.crown.getFinalFlameLength(LengthUnits::Feet);
    reportTestResult(row, testInfo, testName, observedFlameLength, expected.flameLength, error_tolerance);
  }

  if (expected.firelineIntensity != 0) {
    testName = "Test fireline intensity";
    observedFirelineIntensity = behaveRun.crown.getFinalFirelineIntesity(FirelineIntensityUnits::BtusPerFootPerSecond);
    reportTestResult(row, testInfo, testName, observedFirelineIntensity, expected.firelineIntensity, error_tolerance);
  }

  if (expected.fireType != -1) {
    testName = "Test fire type";
    observedFireType = (int)behaveRun.crown.getFireType();
    reportTestResult(row, testInfo, testName, observedFireType, (int)expected.fireType, error_tolerance);
  }

  std::cout << "Finished testing Crown module\n\n";
}
