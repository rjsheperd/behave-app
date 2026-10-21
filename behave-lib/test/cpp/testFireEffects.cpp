// testFireEffects: behave-lib's adapters for the FOFEM modules.
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
} // namespace

int main()
{
    SIGFuelConsumption consumption;
    testFuelConsumptionAdapter(consumption);
    testSoilHeatingAdapter(consumption);
    std::cout << "Total tests performed: " << numberOfTests << "\n";
    std::cout << "Total tests failed: " << numberOfFailures << "\n";
    return numberOfFailures > 0 ? 1 : 0;
}
