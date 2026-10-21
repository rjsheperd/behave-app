// testSoilHeating: SIGSoilHeating (what the app binds), fed by
// SIGFuelConsumption, against FOFEM 6.9.4's soil runs, one row of
// test/csv/soil_heating.csv per plot.
#include <iostream>

#include "SIGSoilHeating.h"
#include "fireEffectsTestInputs.h"

int main(int argc, char* argv[])
{
  if (argc == 1) {
    std::cout << "Unable continue tests. Please supply CSV filename.\n" << std::endl;
    return 1;
  }
  TestInfo testInfo;
  std::vector<std::string> csvHeaders;
  std::vector<std::map<std::string, std::string>> csvStringRows;
  std::vector<std::map<std::string, double>> csvDoubleRows;
  parseCSVFile(argv[1], csvHeaders, csvStringRows, csvDoubleRows);
  std::cout << "Performing tests with: " << csvStringRows.size() << " samples.\n";

  for (int i = 0; i < (int)csvStringRows.size(); i++) {
    std::map<std::string, double>& d = csvDoubleRows[i];
    std::map<std::string, std::string>& s = csvStringRows[i];
    SIGFuelConsumption consumption;
    setFuelConsumptionInputs(consumption, d, s);
    bool ran = consumption.doFuelConsumptionRun();
    SIGSoilHeating soil;
    soil.setSoilFamily(soilFamilyByName.at(s["soilFamily"]));
    soil.setSoilMoisture(d["soilMoisture"], FractionUnits::Percent);
    soil.setInputsFromFuelConsumption(consumption);
    ran = ran && soil.doSoilHeatingRun();
    reportTestResult(i, testInfo, s["plot"] + " runs", ran ? 1 : 0, 1, 0.5);
    if (!ran) {
      continue;
    }
    // Consumption feeds the soil model, so check it too
    for (const auto& o : fuelConsumptionObservations(consumption)) {
      reportTestResult(i, testInfo, s["plot"] + " " + o.first, o.second, d[o.first], fofemTolerance(d[o.first]));
    }
    const TemperatureUnits::TemperatureUnitsEnum celsius = TemperatureUnits::Celsius;
    const std::vector<std::pair<std::string, double>> soilObserved = {
      { "Lay0", soil.getMaximumTemperatureAtLayer(0, celsius) },
      { "Lay2", soil.getMaximumTemperatureAtLayer(2, celsius) },
      { "Lay4", soil.getMaximumTemperatureAtLayer(4, celsius) },
      { "Lay6", soil.getMaximumTemperatureAtLayer(6, celsius) },
      { "Lay60d", (double)soil.getDeepestLayerReachingSixtyDegreesCelsius() },
      { "Lay275d", (double)soil.getDeepestLayerReachingTwoHundredSeventyFiveDegreesCelsius() } };
    for (const auto& o : soilObserved) {
      reportTestResult(i, testInfo, s["plot"] + " " + o.first, o.second, d[o.first], fofemTolerance(d[o.first]));
    }
  }
  std::cout << "Total tests performed: " << testInfo.numTotalTests << "\n";
  std::cout << "Total tests passed: " << testInfo.numPassed << "\n";
  std::cout << "Total tests failed: " << testInfo.numFailed << "\n\n";
  return testInfo.numFailed > 0 ? 1 : 0;
}
