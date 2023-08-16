#include <iostream>
#include <fstream>
#include <type_traits>
#include <typeindex>
#include <nlohmann/json.hpp>
#include "behaveUnits.cpp"

class UnitResolver {
public:
  void addUnit(const std::string& unit, int value) {
    units_[unit] = value;
  }

  template<typename Enum>
  Enum resolveUnit(const std::string& unit) {
    return static_cast<Enum>(units_[unit]);
  }

private:
  std::unordered_map<std::string, int> units_;
};

void addUnits(UnitResolver &unitResolver) {

  unitResolver.addUnit("%",           CoverUnits::Percent);
  unitResolver.addUnit("deg",         SlopeUnits::Degrees);
  unitResolver.addUnit("fraction",    CoverUnits::Fraction);
  // unitResolver.addUnit("points"); // FIXME Contain Fire Points
  // unitResolver.addUnit("ratio"); // FIXME
  unitResolver.addUnit("Btu/ft/s",    FirelineIntensityUnits::BtusPerFootPerSecond);
  unitResolver.addUnit("Btu/ft/min",  FirelineIntensityUnits::BtusPerFootPerMinute);
  unitResolver.addUnit("Btu/ft2",     HeatPerUnitAreaUnits::BtusPerSquareFoot);
  unitResolver.addUnit("Btu/ft2/min", HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerMinute);
  unitResolver.addUnit("Btu/ft2/sec", HeatSourceAndReactionIntensityUnits::BtusPerSquareFootPerSecond);
  unitResolver.addUnit("Btu/ft3",     HeatSinkUnits::BtusPerCubicFoot);
  unitResolver.addUnit("Btu/lb",      HeatOfCombustionUnits::BtusPerPound);
  unitResolver.addUnit("ac",          AreaUnits::Acres);
  unitResolver.addUnit("ch",          LengthUnits::Chains);
  unitResolver.addUnit("ch/h",        SpeedUnits::ChainsPerHour);
  unitResolver.addUnit("ft",          LengthUnits::Feet);
  // unitResolver.addUnit("ft-lb/s/ft2"); // FIXME Power Units
  unitResolver.addUnit("ft/min",      SpeedUnits::FeetPerMinute);
  unitResolver.addUnit("ft2",         AreaUnits::SquareFeet);
  // unitResolver.addUnit("ft2/ac"); // FIXME Basal Area Units
  unitResolver.addUnit("ft2/ft3",     SurfaceAreaToVolumeUnits::SquareFeetOverCubicFeet);
  unitResolver.addUnit("in",          LengthUnits::Inches);
  unitResolver.addUnit("lb/ft3",      DensityUnits::PoundsPerCubicFoot);
  unitResolver.addUnit("lbs/ft3",     DensityUnits::PoundsPerCubicFoot);
  unitResolver.addUnit("mi",          LengthUnits::Miles);
  unitResolver.addUnit("mi/h",        SpeedUnits::MilesPerHour);
  // unitResolver.addUnit("ms"); // FIXME
  unitResolver.addUnit("oF",          TemperatureUnits::Fahrenheit);
  // unitResolver.addUnit("per        ac"); // FIXME Tree Count
  unitResolver.addUnit("ton/ac",      LoadingUnits::TonsPerAcre);
  unitResolver.addUnit("cm",          LengthUnits::Centimeters);
  unitResolver.addUnit("ha",          AreaUnits::Hectares);
  unitResolver.addUnit("kJ/kg",       HeatOfCombustionUnits::KilojoulesPerKilogram);
  unitResolver.addUnit("kJ/m2",       HeatPerUnitAreaUnits::KilojoulesPerSquareMeter);
  unitResolver.addUnit("kJ/m3",       HeatSinkUnits::KilojoulesPerCubicMeter);
  unitResolver.addUnit("kW/m",        FirelineIntensityUnits::KilowattsPerMeter);
  unitResolver.addUnit("kW/m2",       HeatSourceAndReactionIntensityUnits::KilowattsPerSquareMeter);
  unitResolver.addUnit("kg/m3",       DensityUnits::KilogramsPerCubicMeter);
  unitResolver.addUnit("km",          LengthUnits::Kilometers);
  unitResolver.addUnit("km/h",        SpeedUnits::KilometersPerHour);
  unitResolver.addUnit("m",           LengthUnits::Meters);
  unitResolver.addUnit("m/h",         SpeedUnits::MetersPerHour); // FIXME
  unitResolver.addUnit("m/min",       SpeedUnits::MetersPerMinute);
  unitResolver.addUnit("m2",          AreaUnits::SquareMeters);
  // unitResolver.addUnit("m2/ha"); // FIXME Basal Area
  unitResolver.addUnit("m2/m3",       SurfaceAreaToVolumeUnits::SquareMetersOverCubicMeters);
  unitResolver.addUnit("mm",          LengthUnits::Millimeters);
  unitResolver.addUnit("oC",          TemperatureUnits::Celsius);
  // unitResolver.addUnit("per        ha"); // FIXME Tree Density
  unitResolver.addUnit("tonne/ha",    LoadingUnits::TonnesPerHectare);
  unitResolver.addUnit("s",           TimeUnits::Seconds);
  unitResolver.addUnit("min",         TimeUnits::Minutes);
  unitResolver.addUnit("h",           TimeUnits::Hours);
  unitResolver.addUnit("days",        TimeUnits::Days);
  unitResolver.addUnit("years",       TimeUnits::Years);
}

class FuncConverter {
private:
  UnitResolver _unitResolver;
public:
  FuncConverter(UnitResolver &unitResolver) : _unitResolver(unitResolver) {}

  // Discrete Setter
  template<typename ObjType, typename EnumType>
  void wrapperDiscreteSetter(ObjType* obj, void (ObjType::*func)(EnumType), std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = static_cast<EnumType>(std::stoi(str));

    // Call the member function of the object with the converted EnumType
    (obj->*func)(enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, std::string)> convertDiscreteSetter(void (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      wrapperDiscreteSetter(static_cast<ObjType*>(obj), func, str);
    };
  }

  // Continous Setter
  template<typename ObjType, typename EnumType>
  void wrapperContinuousSetter(ObjType* obj, void (ObjType::*func)(double, EnumType), double num, std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    (obj->*func)(num, enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, double, std::string)> convertContinuousSetter(void (ObjType::*func)(double, EnumType)) {
    return [=](void* obj, double num, std::string str) {
      wrapperContinuousSetter(static_cast<ObjType*>(obj), func, num, str);
    };
  }

  // Calculate Wrapper
  template<typename ObjType>
  std::function<void(void*)> convertCalculate(void (ObjType::*func)()) {
    return [=](void* obj, std::string str) {
      (obj->*func)();
    };
  }

  // Getters
  template<typename ObjType, typename EnumType>
  double wrapperGetter(ObjType* obj, double (ObjType::*func)(EnumType), std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    return (obj->*func)(enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<double(void*, std::string)> convertGetter(double (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      return wrapperGetter(static_cast<ObjType*>(obj), func, str);
    };
  }
};

class SIGTestClass {
public:
  void setHeight(double height, LengthUnits::LengthUnitsEnum units) {
    height_ = LengthUnits::toBaseUnits(height, units);
  };

  double getHeight(LengthUnits::LengthUnitsEnum units) {
    return LengthUnits::fromBaseUnits(height_, units);
  };

  void setSpeed(double speed, SpeedUnits::SpeedUnitsEnum units) {
    speed_ = SpeedUnits::toBaseUnits(speed, units);
  };

  double getSpeed(SpeedUnits::SpeedUnitsEnum units) {
    return SpeedUnits::fromBaseUnits(speed_, units);
  };

  void setUnits(LengthUnits::LengthUnitsEnum units) {
    units_ = units;
  }

  LengthUnits::LengthUnitsEnum getUnits() {
    return units_;
  }

private:
  double height_;
  double speed_;
  LengthUnits::LengthUnitsEnum units_;
};

int main() {
  std::unordered_map<std::string, std::function<void(void*, std::string)>> discrete_setters;
  std::unordered_map<std::string, std::function<void(void*, double, std::string)>> cont_setters;
  std::unordered_map<std::string, std::function<double(void*, std::string)>> getters;

  UnitResolver unitResolver;
  addUnits(unitResolver);

  FuncConverter fnConverter(unitResolver);

  SIGTestClass sut;

  // sut.setSpeed(30.0, SpeedUnits::MilesPerHour);
  cont_setters["setHeight"] = fnConverter.convertContinuousSetter(&SIGTestClass::setHeight);
  cont_setters["setHeight"](&sut, 10.0, "mi");

  cont_setters["setSpeed"] = fnConverter.convertContinuousSetter(&SIGTestClass::setSpeed);
  cont_setters["setSpeed"](&sut, 30.0, "mi/h");

  discrete_setters["setUnits"] = fnConverter.convertDiscreteSetter(&SIGTestClass::setUnits);
  discrete_setters["setUnits"](&sut, "3");

  getters["getSpeed"] = fnConverter.convertGetter(&SIGTestClass::getSpeed);
  std::cout << getters["getSpeed"](&sut, "ft/min") << std::endl;
  std::cout << sut.getSpeed(SpeedUnits::FeetPerMinute) << std::endl;

  getters["getHeight"] = fnConverter.convertGetter(&SIGTestClass::getHeight);
  std::cout << getters["getHeight"](&sut, "ft") << std::endl;
  std::cout << sut.getHeight(LengthUnits::Feet) << std::endl;

  std::cout << sut.getUnits() << std::endl;

  return 0;
}
