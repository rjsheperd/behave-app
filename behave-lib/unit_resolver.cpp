#include <iostream>
#include <sstream>
#include <functional>
#include <variant>
#include <fstream>
#include <type_traits>
#include <typeinfo>
#include <typeindex>
#include <nlohmann/json.hpp>
#include "behaveUnits.cpp"
#include "fuelModels.cpp"
#include "surface.h"
#include "surfaceFuelbedIntermediates.h"






class UnitResolver {
public:
  void addUnit(const std::string& unit, int value) {
    units_[unit] = value;
  }

  template<typename Enum>
  int resolveUnit(const std::string& unit) {
    return static_cast<Enum>(units_[unit]);
  }

private:
  std::unordered_map<std::string, int> units_;
};

void addUnits(UnitResolver &unitResolver) {

  unitResolver.addUnit("%", CoverUnits::Percent);
  unitResolver.addUnit("deg", SlopeUnits::Degrees);
  unitResolver.addUnit("fraction", CoverUnits::Fraction);
  // unitResolver.addUnit("points"); // FIXME Contain Fire Points
  // unitResolver.addUnit("ratio"); // FIXME
  unitResolver.addUnit("Btu/ft/s", FirelineIntensityUnits::BtusPerFootPerSecond);
  unitResolver.addUnit("Btu/ft/min", FirelineIntensityUnits::BtusPerFootPerMinute);
  unitResolver.addUnit("Btu/ft2", HeatPerUnitAreaUnits::BtusPerSquareFoot);
  unitResolver.addUnit("Btu/ft2/min", HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerMinute);
  unitResolver.addUnit("Btu/ft2/sec", HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerSecond);
  unitResolver.addUnit("Btu/ft3", HeatSinkUnits::BtusPerCubicFoot);
  unitResolver.addUnit("Btu/lb", HeatOfCombustionUnits::BtusPerPound);
  unitResolver.addUnit("ac", AreaUnits::Acres);
  unitResolver.addUnit("ch", LengthUnits::Chains);
  unitResolver.addUnit("ch/h", SpeedUnits::ChainsPerHour);
  unitResolver.addUnit("ft", LengthUnits::Feet);
  // unitResolver.addUnit("ft-lb/s/ft2"); // FIXME Power Units
  unitResolver.addUnit("ft/min", SpeedUnits::FeetPerMinute);
  unitResolver.addUnit("ft2", AreaUnits::SquareFeet);
  // unitResolver.addUnit("ft2/ac"); // FIXME Basal Area Units
  unitResolver.addUnit("ft2/ft3", SurfaceAreaToVolumeUnits::SquareFeetOverCubicFeet);
  unitResolver.addUnit("in", LengthUnits::Inches);
  unitResolver.addUnit("lb/ft3", DensityUnits::PoundsPerCubicFoot);
  unitResolver.addUnit("lbs/ft3", DensityUnits::PoundsPerCubicFoot);
  unitResolver.addUnit("mi", LengthUnits::Miles);
  unitResolver.addUnit("mi/h", SpeedUnits::MilesPerHour);
  // unitResolver.addUnit("ms"); // FIXME
  unitResolver.addUnit("oF", TemperatureUnits::Fahrenheit);
  // unitResolver.addUnit("per, ac"); // FIXME Tree Count
  unitResolver.addUnit("ton/ac", LoadingUnits::TonsPerAcre);
  unitResolver.addUnit("cm", LengthUnits::Centimeters);
  unitResolver.addUnit("ha", AreaUnits::Hectares);
  unitResolver.addUnit("kJ/kg", HeatOfCombustionUnits::KilojoulesPerKilogram);
  unitResolver.addUnit("kJ/m2", HeatPerUnitAreaUnits::KilojoulesPerSquareMeter);
  unitResolver.addUnit("kJ/m3", HeatSinkUnits::KilojoulesPerCubicMeter);
  unitResolver.addUnit("kW/m", FirelineIntensityUnits::KilowattsPerMeter);
  unitResolver.addUnit("kW/m2", HeatSourceAndReactionIntensityUnits::KilowattsPerSquareMeter);
  unitResolver.addUnit("kg/m3", DensityUnits::KilogramsPerCubicMeter);
  unitResolver.addUnit("km", LengthUnits::Kilometers);
  unitResolver.addUnit("km/h", SpeedUnits::KilometersPerHour);
  unitResolver.addUnit("m", LengthUnits::Meters);
  unitResolver.addUnit("m/h", SpeedUnits::MetersPerHour); // FIXME
  unitResolver.addUnit("m/min", SpeedUnits::MetersPerMinute);
  unitResolver.addUnit("m2", AreaUnits::SquareMeters);
  // unitResolver.addUnit("m2/ha"); // FIXME Basal Area
  unitResolver.addUnit("m2/m3", SurfaceAreaToVolumeUnits::SquareMetersOverCubicMeters);
  unitResolver.addUnit("mm", LengthUnits::Millimeters);
  unitResolver.addUnit("oC", TemperatureUnits::Celsius);
  // unitResolver.addUnit("per, ha"); // FIXME Tree Density
  unitResolver.addUnit("tonne/ha", LoadingUnits::TonnesPerHectare);
  unitResolver.addUnit("s", TimeUnits::Seconds);
  unitResolver.addUnit("min", TimeUnits::Minutes);
  unitResolver.addUnit("h", TimeUnits::Hours);
  unitResolver.addUnit("days", TimeUnits::Days);
  unitResolver.addUnit("years", TimeUnits::Years);
}
