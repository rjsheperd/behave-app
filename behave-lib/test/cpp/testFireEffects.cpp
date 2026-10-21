// testFireEffects: behave-lib's adapters for the FOFEM modules, and
// SIGMortality's full-name methods.
// SIGFuelConsumption and SIGSoilHeating forward their inputs' setters and
// getters, so the app can call them on the module. This runs one plot
// through the adapters and through the behave-mirror classes and requires
// the same results. (The models themselves are checked against FOFEM in
// behave-mirror's testFofem.) Exits non-zero when a test fails.
#include <cmath>
#include <cstring>
#include <iostream>
#include <string>

#include "SIGFuelConsumption.h"
#include "SIGMortality.h"
#include "SIGSoilHeating.h"

namespace
{
int numberOfTests = 0;
int numberOfFailures = 0;

void check(const std::string& name, bool passed)
{
    numberOfTests++;
    if (!passed)
    {
        numberOfFailures++;
        std::cout << "FAILED " << name << "\n";
    }
}

// A western plot with every kind of fuel; T is SIGFuelConsumption or
// FuelConsumptionInputs, which share these method names
template <class T>
void setPlot(T& t)
{
    const LoadingUnits::LoadingUnitsEnum tpa = LoadingUnits::TonsPerAcre;
    t.setRegion(FuelConsumptionRegion::InteriorWest);
    t.setCoverGroup(CoverGroup::PonderosaPine);
    t.setBurnSeason(BurnSeason::Summer);
    t.setFuelCategory(FuelCategory::Natural);
    t.setFuelLoadLitter(1.0, tpa);
    t.setFuelLoadOneHour(0.5, tpa);
    t.setFuelLoadTenHour(1.0, tpa);
    t.setFuelLoadHundredHour(2.0, tpa);
    t.distributeThousandHourFuelLoad(10.0, tpa, 30.0, FractionUnits::Percent,
        ThousandHourLoadDistribution::Even);
    t.setFuelLoadDuff(20.0, tpa);
    t.setDuffDepth(1.5, LengthUnits::Inches);
    t.setFuelLoadHerbaceous(0.3, tpa);
    t.setFuelLoadShrub(0.6, tpa);
    t.setFuelLoadCrownFoliage(0.0, tpa);
    t.setFuelLoadCrownBranch(0.0, tpa);
    t.setMoistureDuff(60.0, FractionUnits::Percent);
    t.setMoistureTenHour(12.0, FractionUnits::Percent);
    t.setMoistureThousandHour(20.0, FractionUnits::Percent);
}

void testFuelConsumptionAdapter(SIGFuelConsumption& adapter)
{
    std::cout << "Testing SIGFuelConsumption\n";
    setPlot(adapter);
    FuelConsumption direct;
    setPlot(direct.getInputs());
    check("adapter run", adapter.doFuelConsumptionRun());
    check("direct run", direct.doFuelConsumptionRun());
    check("forwarded getter reads the input",
        adapter.getFuelLoadDuff(LoadingUnits::TonsPerAcre) == 20.0
        && adapter.getRegion() == FuelConsumptionRegion::InteriorWest);
    int mismatches = 0;
    for (int c = 0; c <= FuelConsumptionComponent::Total; c++)
    {
        auto component = static_cast<FuelConsumptionComponent::FuelConsumptionComponentEnum>(c);
        mismatches += adapter.getFuelLoadConsumed(component, LoadingUnits::TonsPerAcre)
            != direct.getFuelLoadConsumed(component, LoadingUnits::TonsPerAcre);
        mismatches += adapter.getFuelLoadPostfire(component, LoadingUnits::TonsPerAcre)
            != direct.getFuelLoadPostfire(component, LoadingUnits::TonsPerAcre);
    }
    for (int e = 0; e <= EmissionSpecies::ParticulateMatterTen; e++)
    {
        auto species = static_cast<EmissionSpecies::EmissionSpeciesEnum>(e);
        mismatches += adapter.getFlamingEmission(species, LoadingUnits::PoundsPerAcre)
            != direct.getFlamingEmission(species, LoadingUnits::PoundsPerAcre);
    }
    mismatches += adapter.getSmolderingDuration(TimeUnits::Seconds) != direct.getSmolderingDuration(TimeUnits::Seconds);
    check("adapter matches FuelConsumption (" + std::to_string(mismatches) + " differ)", mismatches == 0);
    check("some fuel was consumed", adapter.getFuelLoadConsumed(FuelConsumptionComponent::Total, LoadingUnits::TonsPerAcre) > 0);

    SIGFuelConsumption copy(adapter);
    check("a copy keeps the results",
        copy.getFuelLoadConsumed(FuelConsumptionComponent::Duff, LoadingUnits::TonsPerAcre)
        == adapter.getFuelLoadConsumed(FuelConsumptionComponent::Duff, LoadingUnits::TonsPerAcre));

    SIGFuelConsumption invalid;
    invalid.setMoistureTenHour(-5.0, FractionUnits::Percent);
    bool ran = invalid.doFuelConsumptionRun();
    char* message = invalid.getErrorMessage();
    check("an invalid input fails with a char* message", !ran && message != nullptr && std::strlen(message) > 0);
    delete[] message;
}

void testSoilHeatingAdapter(const SIGFuelConsumption& consumption)
{
    std::cout << "Testing SIGSoilHeating\n";
    SIGSoilHeating adapter;
    adapter.setSoilFamily(SoilFamily::LoamySkeletal);
    adapter.setSoilMoisture(10.0, FractionUnits::Percent);
    adapter.setInputsFromFuelConsumption(consumption);
    SoilHeating direct;
    direct.getInputs().setSoilFamily(SoilFamily::LoamySkeletal);
    direct.getInputs().setSoilMoisture(10.0, FractionUnits::Percent);
    direct.setInputsFromFuelConsumption(consumption);
    check("adapter run", adapter.doSoilHeatingRun());
    check("direct run", direct.doSoilHeatingRun());
    check("forwarded getters read the inputs",
        adapter.getSoilFamily() == SoilFamily::LoamySkeletal
        && adapter.getSoilMoisture(FractionUnits::Percent) == 10.0
        && adapter.getNumberOfFireIntensities() == direct.getInputs().getNumberOfFireIntensities());
    int mismatches = 0;
    for (int layer = 0; layer < SoilHeating::NumberOfLayers; layer++)
    {
        mismatches += adapter.getMaximumTemperatureAtLayer(layer, TemperatureUnits::Celsius)
            != direct.getMaximumTemperatureAtLayer(layer, TemperatureUnits::Celsius);
    }
    check("adapter matches SoilHeating (" + std::to_string(mismatches) + " differ)", mismatches == 0);
    check("the surface heats", adapter.getMaximumTemperatureAtLayer(0, TemperatureUnits::Celsius) > 60.0);
    char* message = adapter.getMessage();
    check("the message is a char*", message != nullptr);
    delete[] message;
    SIGSoilHeating copy(adapter);
    check("a copy keeps the results",
        copy.getMaximumTemperatureAtLayer(0, TemperatureUnits::Celsius)
        == adapter.getMaximumTemperatureAtLayer(0, TemperatureUnits::Celsius));
}
// SIGMortality's full names (behave Mortality's renames, kept under the old
// names too until the app has moved)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
void testMortalityNames()
{
    std::cout << "Testing SIGMortality's full names\n";
    SpeciesMasterTable table;
    // ABAM uses crown scorch equation 1, whose bark thickness comes from DBH
    auto tree = [&table](bool fullNames, double dbh) {
        SIGMortality mortality(table);
        char species[] = "ABAM";
        mortality.setEquationType(EquationType::crown_scorch);
        mortality.setSpeciesCode(species);
        mortality.updateInputsForSpeciesCodeAndEquationType(species, EquationType::crown_scorch);
        if (fullNames)
        {
            mortality.setDiameterAtBreastHeight(dbh, LengthUnits::Inches);
        }
        else
        {
            mortality.setDBH(dbh, LengthUnits::Inches);
        }
        mortality.setTreeHeight(60, LengthUnits::Feet);
        mortality.setCrownRatio(0.5, FractionUnits::Fraction);
        mortality.setSurfaceFireScorchHeight(45, LengthUnits::Feet);
        mortality.calculateMortalityAllDirections(FractionUnits::Fraction);
        return mortality.getProbabilityOfMortality(FractionUnits::Fraction);
    };
    double full = tree(true, 12);
    check("setDiameterAtBreastHeight reaches the heading tree", full > 0 && tree(true, 30) != full);
    check("setDBH and setDiameterAtBreastHeight agree", tree(false, 12) == full);
    SIGMortality mortality(table);
    SpeciesMasterTableRecordVector* a = mortality.getSpeciesRecordVectorForGeographicAreaCoordinationCenter(GACC::Northwest);
    SpeciesMasterTableRecordVector* b = mortality.getSpeciesRecordVectorForGACCRegion(GACC::Northwest);
    check("the species vector under both names", a->vec.size() == b->vec.size() && a->vec.size() > 0);
    delete a;
    delete b;
}
#pragma GCC diagnostic pop
} // namespace

int main()
{
    SIGFuelConsumption consumption;
    testFuelConsumptionAdapter(consumption);
    testSoilHeatingAdapter(consumption);
    testMortalityNames();
    std::cout << "Total tests performed: " << numberOfTests << "\n";
    std::cout << "Total tests failed: " << numberOfFailures << "\n";
    return numberOfFailures > 0 ? 1 : 0;
}
