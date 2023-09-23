#pragma once

#include <string>
#include <unordered_map>
#include "behaveUnits.cpp"

class UnitResolver {
private:
  std::unordered_map<std::string, int> units_;
public:
  UnitResolver() {
    addUnit(AreaUnits::Acres, "ac");
    addUnit(AreaUnits::Hectares, "ha");
    addUnit(AreaUnits::SquareFeet, "ft2");
    addUnit(AreaUnits::SquareMeters, "m2");

    addUnit(DensityUnits::KilogramsPerCubicMeter, "kg/m3");
    addUnit(DensityUnits::PoundsPerCubicFoot, "lb/ft3");
    addUnit(DensityUnits::PoundsPerCubicFoot, "lbs/ft3");

    addUnit(FirelineIntensityUnits::BtusPerFootPerMinute, "Btu/ft/min");
    addUnit(FirelineIntensityUnits::BtusPerFootPerSecond, "Btu/ft/s");
    addUnit(FirelineIntensityUnits::KilowattsPerMeter, "kW/m");

    addUnit(FractionUnits::Fraction, "fraction");
    addUnit(FractionUnits::Percent, "%");

    addUnit(HeatOfCombustionUnits::BtusPerPound, "Btu/lb");
    addUnit(HeatOfCombustionUnits::KilojoulesPerKilogram, "kJ/kg");

    addUnit(HeatPerUnitAreaUnits::BtusPerSquareFoot, "Btu/ft2");
    addUnit(HeatPerUnitAreaUnits::KilojoulesPerSquareMeter, "kJ/m2");

    addUnit(HeatSinkUnits::BtusPerCubicFoot, "Btu/ft3");
    addUnit(HeatSinkUnits::KilojoulesPerCubicMeter, "kJ/m3");

    addUnit(HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerMinute, "Btu/ft2/min");
    addUnit(HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerSecond, "Btu/ft2/sec");
    addUnit(HeatSourceAndReactionIntensityUnits::KilowattsPerSquareMeter, "kW/m2");

    addUnit(LengthUnits::Centimeters, "cm");
    addUnit(LengthUnits::Chains, "ch");
    addUnit(LengthUnits::Feet, "ft");
    addUnit(LengthUnits::Inches, "in");
    addUnit(LengthUnits::Kilometers, "km");
    addUnit(LengthUnits::Meters, "m");
    addUnit(LengthUnits::Miles, "mi");
    addUnit(LengthUnits::Millimeters, "mm");

    addUnit(LoadingUnits::TonnesPerHectare, "tonne/ha");
    addUnit(LoadingUnits::TonsPerAcre, "ton/ac");

    addUnit(SlopeUnits::Degrees, "deg");

    addUnit(SpeedUnits::ChainsPerHour, "ch/h");
    addUnit(SpeedUnits::FeetPerMinute, "ft/min");
    addUnit(SpeedUnits::KilometersPerHour, "km/h");
    addUnit(SpeedUnits::MetersPerHour, "m/h"); // FIXME
    addUnit(SpeedUnits::MetersPerMinute, "m/min");
    addUnit(SpeedUnits::MilesPerHour, "mi/h");

    addUnit(SurfaceAreaToVolumeUnits::SquareFeetOverCubicFeet, "ft2/ft3");
    addUnit(SurfaceAreaToVolumeUnits::SquareMetersOverCubicMeters, "m2/m3");

    addUnit(TemperatureUnits::Celsius, "oC");
    addUnit(TemperatureUnits::Fahrenheit, "oF");

    addUnit(TimeUnits::Days, "days");
    addUnit(TimeUnits::Hours, "h");
    addUnit(TimeUnits::Minutes, "min");
    addUnit(TimeUnits::Seconds, "s");
    addUnit(TimeUnits::Years, "years");
  }

  void addUnit(int value, const std::string unit) {
    units_[unit] = value;
  }

  template<typename Enum>
  Enum resolveUnit(const std::string unit) {
    return static_cast<Enum>(units_[unit]);
  }
};
