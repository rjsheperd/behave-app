// testFuelConsumption: SIGFuelConsumption (what the app binds) against
// FOFEM 6.9.4, one row of test/csv/fuel_consumption.csv per plot.
#include <iostream>

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
    SIGFuelConsumption run;
    setFuelConsumptionInputs(run, csvDoubleRows[i], csvStringRows[i]);
    bool ran = run.doFuelConsumptionRun();
    reportTestResult(i, testInfo, csvStringRows[i]["plot"] + " runs", ran ? 1 : 0, 1, 0.5);
    if (!ran) {
      continue;
    }
    for (const auto& o : fuelConsumptionObservations(run)) {
      double expected = csvDoubleRows[i][o.first];
      reportTestResult(i, testInfo, csvStringRows[i]["plot"] + " " + o.first, o.second, expected, fofemTolerance(expected));
    }
  }
  std::cout << "Total tests performed: " << testInfo.numTotalTests << "\n";
  std::cout << "Total tests passed: " << testInfo.numPassed << "\n";
  std::cout << "Total tests failed: " << testInfo.numFailed << "\n\n";
  return testInfo.numFailed > 0 ? 1 : 0;
}
