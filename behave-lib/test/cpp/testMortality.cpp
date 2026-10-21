// testMortality: SIGMortality (what the app binds) on test/csv/mortality.csv
// (3798 trees; MortAvgPercent is FOFEM's output rounded to a whole percent).
// Sets inputs as projects/behave's mortality_test.cljs does, with the full
// method names. Exits non-zero on failure.
#include <iostream>

#include "SIGMortality.h"
#include "testUtils.h"

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

  const std::map<std::string, EquationType> equationTypes{
    {"CRNSCH", EquationType::crown_scorch},
    {"CRCABE", EquationType::crown_damage},
    {"BOLCHR", EquationType::bole_char}};
  SpeciesMasterTable table;

  for (int i = 0; i < (int)csvStringRows.size(); i++) {
    std::map<std::string, double>& d = csvDoubleRows[i];
    std::map<std::string, std::string>& s = csvStringRows[i];
    auto has = [&d](const char* key) { return d.count(key) > 0; };
    SIGMortality mortality(table);
    std::vector<char> species(s["TreeSpecies"].begin(), s["TreeSpecies"].end());
    species.push_back('\0');
    EquationType equationType = equationTypes.count(s["EquationType"]) ? equationTypes.at(s["EquationType"]) : EquationType::not_set;
    mortality.setGeographicAreaCoordinationCenter(GACC::SouthernArea);
    mortality.setEquationType(equationType);
    mortality.setSpeciesCode(species.data());
    mortality.updateInputsForSpeciesCodeAndEquationType(species.data(), equationType);
    if (s["FS"].empty()) {
      mortality.setSurfaceFireFlameLength(4, LengthUnits::Feet);
    } else if (s["FS"] == "S" && has("FlLe/ScHt")) {
      mortality.setSurfaceFireScorchHeight(d["FlLe/ScHt"], LengthUnits::Feet);
    } else if (has("FlLe/ScHt")) {
      mortality.setSurfaceFireFlameLength(d["FlLe/ScHt"], LengthUnits::Feet);
    }
    if (has("TreeExpansionFactor")) mortality.setTreeDensityPerUnitArea(d["TreeExpansionFactor"], AreaUnits::Acres);
    if (has("Diameter")) mortality.setDiameterAtBreastHeight(d["Diameter"], LengthUnits::Inches);
    if (has("TreeHeight")) mortality.setTreeHeight(d["TreeHeight"], LengthUnits::Feet);
    if (has("CrownRatio")) mortality.setCrownRatio(d["CrownRatio"] / 100, FractionUnits::Fraction);
    if (has("CrownScorch%")) mortality.setCrownDamage(d["CrownScorch%"]);
    if (has("CKR")) mortality.setCambiumKillRating(d["CKR"]);
    if (!s["BeetleDamage"].empty()) mortality.setBeetleDamage(s["BeetleDamage"] == "Yes" || s["BeetleDamage"] == "YES" ? BeetleDamage::yes : BeetleDamage::no);
    if (has("BoleCharHeight")) mortality.setBoleCharHeight(d["BoleCharHeight"], LengthUnits::Feet);
    mortality.calculateMortalityAllDirections(FractionUnits::Percent);
    // The expected value is rounded to a whole percent
    reportTestResult(i, testInfo, s["PlotId"] + " mortality", mortality.getProbabilityOfMortality(FractionUnits::Percent),
                     d["MortAvgPercent"], 0.5 + 1.0e-9);
  }
  std::cout << "Total tests performed: " << testInfo.numTotalTests << "\n";
  std::cout << "Total tests passed: " << testInfo.numPassed << "\n";
  std::cout << "Total tests failed: " << testInfo.numFailed << "\n\n";
  return testInfo.numFailed > 0 ? 1 : 0;
}
