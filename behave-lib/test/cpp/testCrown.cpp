#DEFINE BOOST_TEST_MODULE Crown

#include <boost/test/included/unit_test.hpp>
#include <iostream>
#include <sstream>
#include <vector>
#include <utility>
#include "behaveRun.h"
#include "fuelModels.h"
#include "testUtils.h"

using namespace boost::unit_test;

// Define the error tolerance for double values
constexpr double error_tolerance = 1e-06;

constexpr bool DEBUG = 0;

// Testing Structs
struct CrownTestInputs {
  std::string testID = "";
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
void testCrownModule(CrownTestInputs& crownInputs, CrownTestOutputs& crownOutputs, BehaveRun& behaveRun);

typedef std::tuple<int, CrownTestInputs, CrownTestOutputs> TestTuple;

void convertCSVDataToCrownIO(CSVData * csvData, std::vector<TestTuple> * allCrownIO) {

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

  // Perform Tests using CSV Inputs
  for (int i = 0; i < csvData->csvStringRows.size(); i++) {

    // Init structs
    CrownTestInputs crownInputs;
    CrownTestOutputs crownOutputs;

    // Set up Inputs
    crownInputs.testID = csvData->csvStringRows[i]["testID"];
    crownInputs.fuelModelNumber = csvData->csvDoubleRows[i]["fuelModelNumber"];
    crownInputs.moistureOneHour = csvData->csvDoubleRows[i]["moistureOneHour"];
    crownInputs.moistureTenHour = csvData->csvDoubleRows[i]["moistureTenHour"];
    crownInputs.moistureHundredHour = csvData->csvDoubleRows[i]["moistureHundredHour"];
    crownInputs.moistureLiveHerbaceous = csvData->csvDoubleRows[i]["moistureLiveHerbaceous"];
    crownInputs.moistureLiveWoody = csvData->csvDoubleRows[i]["moistureLiveWoody"];
    crownInputs.moistureFoliar = csvData->csvDoubleRows[i]["moistureFoliar"];
    crownInputs.moistureUnits = moistureUnits[csvData->csvStringRows[i]["moistureUnits"]];
    crownInputs.windSpeed = csvData->csvDoubleRows[i]["windSpeed"];
    crownInputs.windSpeedUnits = speedUnits[csvData->csvStringRows[i]["windSpeedUnits"]];
    crownInputs.windHeightInputMode = windHeightInputMode[csvData->csvStringRows[i]["windHeightInputMode"]];
    crownInputs.windDirection = csvData->csvDoubleRows[i]["windDirection"];
    crownInputs.windAndSpreadOrientationMode = windAndSpreadOrientationMode[csvData->csvStringRows[i]["windAndSpreadOrientationMode"]];
    crownInputs.windAdjustmentFactor = csvData->csvDoubleRows[i]["windAdjFactor"];
    crownInputs.windAdjustmentFactorCalculationMethod = windAdjustmentFactorCalculationMethod[csvData->csvStringRows[i]["windAdjFactorCalcMethod"]];
    crownInputs.slope = csvData->csvDoubleRows[i]["slope"];
    crownInputs.slopeUnits = slopeUnits[csvData->csvStringRows[i]["slopeUnits"]];
    crownInputs.aspect = csvData->csvDoubleRows[i]["aspect"];
    crownInputs.canopyCover = csvData->csvDoubleRows[i]["canopyCover"];
    crownInputs.coverUnits = coverUnits[csvData->csvStringRows[i]["coverUnits"]];
    crownInputs.canopyHeight = csvData->csvDoubleRows[i]["canopyHeight"];
    crownInputs.canopyBaseHeight = csvData->csvDoubleRows[i]["canopyBaseHeight"];
    crownInputs.canopyHeightUnits = lengthUnits[csvData->csvStringRows[i]["canopyHeightUnits"]];
    crownInputs.crownRatio = csvData->csvDoubleRows[i]["crownRatio"];
    crownInputs.canopyBulkDensity = csvData->csvDoubleRows[i]["canopyBulkDensity"];
    crownInputs.canopyBulkDensityUnits = densityUnits[csvData->csvStringRows[i]["canopyBulkDensityUnits"]];

    // Set up Outputs
    crownOutputs.lengthToWidthRatio = csvData->csvDoubleRows[i]["lengthToWidthRatio"];
    crownOutputs.fireSpreadRate = csvData->csvDoubleRows[i]["fireSpreadRate"];
    crownOutputs.flameLength = csvData->csvDoubleRows[i]["flameLength"];
    crownOutputs.firelineIntensity = csvData->csvDoubleRows[i]["firelineIntensity"];
    crownOutputs.fireType = fireType[csvData->csvStringRows[i]["fireType"]];

    // Add to results
    TestTuple newIO(i, crownInputs, crownOutputs);
    allCrownIO->push_back(newIO);
  }
}

void testCrownModule(CrownTestInputs const& inputs, CrownTestOutputs const& expected, BehaveRun& behaveRun)
{
  std::cout << "Testing Crown module\n";

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
    std::stringstream testName;
    if (!inputs.testID.empty()) { testName << "[ID: " << inputs.testID << " ]"; }
    testName << " Test length-to-width ratio";
    observedLengthToWidthRatio = behaveRun.crown.getCrownFireLengthToWidthRatio();
    reportTestResult(testName.str(), observedSpreadRate, expected.fireSpreadRate, error_tolerance);
  }

  if (expected.fireSpreadRate != 0) {
    std::stringstream testName;
    if (!inputs.testID.empty()) { testName << "[ID: " << inputs.testID << " ]"; }
    testName << " Test fire spread rate";
    observedSpreadRate = behaveRun.crown.getFinalSpreadRate(SpeedUnits::ChainsPerHour);
    reportTestResult(testName.str(), observedSpreadRate, expected.fireSpreadRate, error_tolerance);
  }

  if (expected.flameLength != 0) {
    std::stringstream testName;
    if (!inputs.testID.empty()) { testName << "[ID: " << inputs.testID << " ]"; }
    testName << " Test flame length";
    observedFlameLength = behaveRun.crown.getFinalFlameLength(LengthUnits::Feet);
    reportTestResult(testName.str(), observedFlameLength, expected.flameLength, error_tolerance);
  }

  if (expected.firelineIntensity != 0) {
    std::stringstream testName;
    if (!inputs.testID.empty()) { testName << "[ID: " << inputs.testID << " ]"; }
    testName << " Test fireline intensity ";
    observedFirelineIntensity = behaveRun.crown.getFinalFirelineIntesity(FirelineIntensityUnits::BtusPerFootPerSecond);
    reportTestResult(testName.str(), observedFirelineIntensity, expected.firelineIntensity, error_tolerance);
  }

  if (expected.fireType != -1) {
    std::stringstream testName;
    if (!inputs.testID.empty()) { testName << "[ID: " << inputs.testID << " ]"; }
    testName << "Test fire type ";
    observedFireType = (int)behaveRun.crown.getFireType();
    reportTestResult(testName.str(), observedFireType, (int)expected.fireType, error_tolerance);
  }

  std::cout << "Finished testing Crown module\n\n";
}

class CSVData {
 public:

  static CSVData* factory(std::string const& filename);

  std::vector<std::string> csvHeaders;
  std::vector<std::map<std::string, std::string>> csvStringRows;
  std::vector<std::map<std::string, double>> csvDoubleRows;
};

CSVData* CSVData::factory(std::string const& filename) {

  CSVData* csvData = new CSVData();

  int result = parseCSVFile(filename, csvData->csvHeaders, csvData->csvStringRows, csvData->csvDoubleRows);

  if (result == 0) {
    return csvData;
  } else {
    return nullptr;
  }
}

/// The interface with the device driver.
struct CrownInit {
  CrownInit() {
    BOOST_TEST_REQUIRE( framework::master_test_suite().argc == 3 );
    BOOST_TEST_REQUIRE( framework::master_test_suite().argv[1] == "--csv-file" );
  }
  void setup() {
    // CSV Parsing
    csvData = CSVData::factory(framework::master_test_suite().argv[2]);
    BOOST_TEST_REQUIRE(
                       csvData != nullptr,
                       "Cannot parse CSV Data from file:" << framework::master_test_suite().argv[2] );

    // Turn CSV Data into Struct
    ioVec = new IOVec();
    convertCSVDataToCrownIO(csvData, ioVec);
    BOOST_TEST_REQUIRE((*ioVec).size() == (csvData->csvHeaders).size(), "Could not convert CSV to Inputs/Outputs.");

    // Create Behave Runner
    behaveRun = new BehaveRun(new FuelModels(), new SpeciesMasterTable());
  }
  void teardown() {
    delete csvData;
    delete ioVec;
    delete behaveRun;
  }

  static CSVData* csvData;
  static IOVec* ioVec;
  static BehaveRun* behaveRun;
};

// Clean start
CSVData* CrownInit::csvData = nullptr;
IOVec* CrownInit::ioVec = nullptr;
BehaveRun *CrownInit::behaveRun = nullptr;

// Fixture
BOOST_TEST_GLOBAL_FIXTURE( CrownInit );

// Iterate over all Vector Pairs of Inputs/Outputs
BOOST_DATA_TEST_CASE(test_crown_only, CrownInit::ioVec, io) {
  CrownTestInputs inputs = std::get<0>(io);
  CrownTestOutputs outputs = std::get<1>(io);
  testCrownModule(inputs, expected, CrownInit::behaveRun);
}
