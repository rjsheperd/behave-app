#include <iostream>
#include <fstream>
#include <type_traits>
#include <typeindex>
#include <nlohmann/json.hpp>
#include "behaveUnits.cpp"

using json = nlohmann::json;

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

  void addUnit(int value, const std::string& unit) {
    units_[unit] = value;
  }

  template<typename Enum>
  Enum resolveUnit(const std::string& unit) {
    return static_cast<Enum>(units_[unit]);
  }
};

// std::to_underlying shim
template <typename E>
constexpr typename std::underlying_type<E>::type to_underlying(E e) noexcept {
    return static_cast<typename std::underlying_type<E>::type>(e);
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
    return [=, this](void* obj, std::string str) {
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
    return [=, this](void* obj, double num, std::string str) {
      wrapperContinuousSetter(static_cast<ObjType*>(obj), func, num, str);
    };
  }

  // Calculate Wrapper
  template<typename ObjType>
  std::function<void(void*)> convertCalculate(void (ObjType::*func)()) {
    return [=, this](void* obj, std::string str) {
      (obj->*func)();
    };
  }

  // Continuous Getters
  template<typename ObjType, typename EnumType>
  double wrapContinuousGetter(ObjType* obj, double (ObjType::*func)(EnumType), std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    return (obj->*func)(enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<double(void*, std::string)> convertContinuousGetter(double (ObjType::*func)(EnumType)) {
    return [=, this](void* obj, std::string str) {
      return wrapContinuousGetter(static_cast<ObjType*>(obj), func, str);
    };
  }

  // Discrete Getters
  template<typename ObjType, typename EnumType>
  int wrapDiscreteGetter(ObjType* obj, EnumType (ObjType::*func)()) {
    // Call the member function of the object with the converted EnumType
    return to_underlying((obj->*func)());
  }

  template<typename ObjType, typename EnumType>
  std::function<int(void*)> convertDiscreteGetter(EnumType (ObjType::*func)()) {
    return [=, this](void* obj) {
      return wrapDiscreteGetter(static_cast<ObjType*>(obj), func);
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
  std::unordered_map<std::string, std::function<double(void*, std::string)>> cont_getters;
  std::unordered_map<std::string, std::function<int(void*)>> disc_getters;

  UnitResolver unitResolver;

  FuncConverter fnConverter(unitResolver);

  SIGTestClass sut;

  // Set Getters/Setters
  cont_setters["setHeight"] = fnConverter.convertContinuousSetter(&SIGTestClass::setHeight);
  cont_setters["setSpeed"] = fnConverter.convertContinuousSetter(&SIGTestClass::setSpeed);
  discrete_setters["setUnits"] = fnConverter.convertDiscreteSetter(&SIGTestClass::setUnits);
  cont_getters["getHeight"] = fnConverter.convertContinuousGetter(&SIGTestClass::getHeight);
  cont_getters["getSpeed"] = fnConverter.convertContinuousGetter(&SIGTestClass::getSpeed);
  disc_getters["getUnits"] = fnConverter.convertDiscreteGetter(&SIGTestClass::getUnits);

  // Read JSON file
  std::ifstream file("data.json");
  if (!file) {
    std::cerr << "Failed to open file!" << std::endl;
    return 1;
  }

  json jsonData;
  file >> jsonData;
  file.close();

  json inputs = jsonData["SIGTestClass"]["inputs"];
  json outputs = jsonData["SIGTestClass"]["outputs"];

  // Apply Inputs
  for (json::iterator it = inputs.begin(); it != inputs.end(); ++it) {

    json input_array = it.value();
    std::string fn_name = it.key();

    if (input_array.size() == 2) {
      double value = input_array[0];
      std::string units = input_array[1];
      std::cout << "ContInput: " << fn_name << " " << value << " " << units << std::endl;
      cont_setters[fn_name](&sut, value, units);
    } else if (input_array.size() == 1) {
      std::string value = input_array[0];
      std::cout << "DiscInput: " << fn_name << " " << value << std::endl;
      discrete_setters[fn_name](&sut, value);
    }
  }

  // Apply Outputs
  for (json::iterator it = outputs.begin(); it != outputs.end(); ++it) {

    std::string fn_name = it.key();
    json value = it.value();

    if (value.is_string()) {
      std::string units = std::string{value};
      std::cout << "ContOutput: " << it.key() << " " << units << std::endl;
      std::cout << cont_getters[fn_name](&sut, units) << std::endl;
    } else {
      std::cout << "TODO DiscOutput: " << it.key() << std::endl;
      std::cout << disc_getters[fn_name](&sut) << std::endl;
    }
  }

  // sut.setSpeed(30.0, SpeedUnits::MilesPerHour);
  //cont_setters["setHeight"](&sut, 10.0, "mi");
  //cont_setters["setSpeed"](&sut, 30.0, "mi/h");
  //discrete_setters["setUnits"](&sut, "3");

  //std::cout << getters["getHeight"](&sut, "ft") << std::endl;

  return 0;
}
