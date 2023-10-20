#include <iostream>
#include <fstream>
#include <type_traits>
#include <typeindex>
#include <typeinfo>
#include <variant>
#include <nlohmann/json.hpp>
#include "behaveUnits.cpp"
#include <sig-adapters/SIGSurface.h>
#include <sig-adapters/SIGContainAdapter.h>
#include <sig-adapters/SIGCrown.h>
#include <sig-adapters/SIGMortality.h>

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

class ClassResolver {
private:
  std::unordered_map<std::string, std::function<void*()>> classMap;

public:
  // Add a class type to the resolver
  template<typename C>
  void addClass(const std::string& name) {
    classMap[name] = []() -> void* { return static_cast<void*>(new C()); };
    // classMap[name] = []() -> void* { return new C(); };
  }

  // Resolve the string to a class type
  void* resolve(const std::string& name) {
    auto it = classMap.find(name);
    if (it != classMap.end()) {
      return (it->second)();
    }
    return nullptr;
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
    return [=](void* obj, std::string str) {
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
    return [=](void* obj) {
      return wrapDiscreteGetter(static_cast<ObjType*>(obj), func);
    };
  }

};

// class SIGTestClass {
// public:
//   void setHeight(double height, LengthUnits::LengthUnitsEnum units) {
//     height_ = LengthUnits::toBaseUnits(height, units);
//   };

//   double getHeight(LengthUnits::LengthUnitsEnum units) {
//     return LengthUnits::fromBaseUnits(height_, units);
//   };

//   void setSpeed(double speed, SpeedUnits::SpeedUnitsEnum units) {
//     speed_ = SpeedUnits::toBaseUnits(speed, units);
//   };

//   double getSpeed(SpeedUnits::SpeedUnitsEnum units) {
//     return SpeedUnits::fromBaseUnits(speed_, units);
//   };

//   void setUnits(LengthUnits::LengthUnitsEnum units) {
//     units_ = units;
//   }

//   LengthUnits::LengthUnitsEnum getUnits() {
//     return units_;
//   }

// private:
//   double height_;
//   double speed_;
//   LengthUnits::LengthUnitsEnum units_;
// };

int main() {
  // Set Classes
  ClassResolver classResolver;
  classResolver.addClass<SIGContainAdapter>("SIGContainAdapter");

  UnitResolver unitResolver;
  FuncConverter fnConverter(unitResolver);

  // Set Getters/Setters
  using Getter0 = std::function<int(void*)>;
  using Getter1 = std::function<double(void*, std::string)>;
  using Calculate0 = std::function<void(void*)>;
  using Setter1 = std::function<void(void*, std::string)>;
  using Setter2 = std::function<void(void*, double, std::string)>;
  using Setter6 = std::function<void(void*, double, double, std::string, double, std::string, std::string)>;
  using FnVariant = std::variant<Getter0, Getter1, Calculate0, Setter1, Setter2>;
  std::unordered_map<std::string, FnVariant> fns;

  // std::cout << GREETING(RJ, true, true);
  // std::cout << ADD_FN(&SIGTestClass, setHeight, true);

  /// Add Setters
  // fns["setHeight"] = fnConverter.convertContinuousSetter(&SIGTestClass::setHeight);
  // fns["setSpeed"] = fnConverter.convertContinuousSetter(&SIGTestClass::setSpeed);
  // fns["setUnits"] = fnConverter.convertDiscreteSetter(&SIGTestClass::setUnits);

  // SIGContainAdapter
  fns["SIGContainAdapter.setAttackDistance"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setAttackDistance);
  fns["SIGContainAdapter.setFireStartTime"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setFireStartTime);
  fns["SIGContainAdapter.setLwRatio"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setLwRatio);
  fns["SIGContainAdapter.setMaxFireSize"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setMaxFireSize);
  fns["SIGContainAdapter.setMaxFireTime"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setMaxFireTime);
  fns["SIGContainAdapter.setMaxSteps"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setMaxSteps);
  fns["SIGContainAdapter.setMinSteps"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setMinSteps);
  fns["SIGContainAdapter.setReportRate"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setReportRate);
  fns["SIGContainAdapter.setReportSize"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setReportSize);
  fns["SIGContainAdapter.setRetry"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setRetry);
  fns["SIGContainAdapter.setTactic"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::setTactic);

  /// SIGSurface
  fns["SIGSurface.setAgeOfRough"] = fnConverter.convertContinuousSetter(&SIGSurface::setAgeOfRough);
  fns["SIGSurface.setAspect"] = fnConverter.convertContinuousSetter(&SIGSurface::setAspect);
  fns["SIGSurface.setAspenCuringLevel"] = fnConverter.convertContinuousSetter(&SIGSurface::setAspenCuringLevel);
  fns["SIGSurface.setAspenDBH"] = fnConverter.convertContinuousSetter(&SIGSurface::setAspenDBH);
  fns["SIGSurface.setAspenFireSeverity"] = fnConverter.convertContinuousSetter(&SIGSurface::setAspenFireSeverity);
  fns["SIGSurface.setAspenFuelModelNumber"] = fnConverter.convertContinuousSetter(&SIGSurface::setAspenFuelModelNumber);
  fns["SIGSurface.setCanopyCover"] = fnConverter.convertContinuousSetter(&SIGSurface::setCanopyCover);
  fns["SIGSurface.setCanopyHeight"] = fnConverter.convertContinuousSetter(&SIGSurface::setCanopyHeight);
  fns["SIGSurface.setChaparralFuelBedDepth"] = fnConverter.convertContinuousSetter(&SIGSurface::setChaparralFuelBedDepth);
  fns["SIGSurface.setChaparralFuelDeadLoadFraction"] = fnConverter.convertContinuousSetter(&SIGSurface::setChaparralFuelDeadLoadFraction);
  fns["SIGSurface.setChaparralFuelLoadInputMode"] = fnConverter.convertContinuousSetter(&SIGSurface::setChaparralFuelLoadInputMode);
  fns["SIGSurface.setChaparralFuelType"] = fnConverter.convertContinuousSetter(&SIGSurface::setChaparralFuelType);
  fns["SIGSurface.setChaparralTotalFuelLoad"] = fnConverter.convertContinuousSetter(&SIGSurface::setChaparralTotalFuelLoad);
  fns["SIGSurface.setCrownRatio"] = fnConverter.convertContinuousSetter(&SIGSurface::setCrownRatio);
  fns["SIGSurface.setDirectionOfInterest"] = fnConverter.convertContinuousSetter(&SIGSurface::setDirectionOfInterest);
  fns["SIGSurface.setElapsedTime"] = fnConverter.convertContinuousSetter(&SIGSurface::setElapsedTime);
  fns["SIGSurface.setFirstFuelModelNumber"] = fnConverter.convertContinuousSetter(&SIGSurface::setFirstFuelModelNumber);
  fns["SIGSurface.setFuelModels"] = fnConverter.convertContinuousSetter(&SIGSurface::setFuelModels);
  fns["SIGSurface.setHeightOfUnderstory"] = fnConverter.convertContinuousSetter(&SIGSurface::setHeightOfUnderstory);
  fns["SIGSurface.setIsUsingChaparral"] = fnConverter.convertContinuousSetter(&SIGSurface::setIsUsingChaparral);
  fns["SIGSurface.setIsUsingPalmettoGallberry"] = fnConverter.convertContinuousSetter(&SIGSurface::setIsUsingPalmettoGallberry);
  fns["SIGSurface.setIsUsingWesternAspen"] = fnConverter.convertContinuousSetter(&SIGSurface::setIsUsingWesternAspen);
  fns["SIGSurface.setMoistureDeadAggregate"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureDeadAggregate);
  fns["SIGSurface.setMoistureHundredHour"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureHundredHour);
  fns["SIGSurface.setMoistureInputMode"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureInputMode);
  fns["SIGSurface.setMoistureLiveAggregate"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureLiveAggregate);
  fns["SIGSurface.setMoistureLiveHerbaceous"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureLiveHerbaceous);
  fns["SIGSurface.setMoistureLiveWoody"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureLiveWoody);
  fns["SIGSurface.setMoistureOneHour"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureOneHour);
  fns["SIGSurface.setMoistureScenarios"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureScenarios);
  fns["SIGSurface.setMoistureTenHour"] = fnConverter.convertContinuousSetter(&SIGSurface::setMoistureTenHour);
  fns["SIGSurface.setOverstoryBasalArea"] = fnConverter.convertContinuousSetter(&SIGSurface::setOverstoryBasalArea);
  fns["SIGSurface.setPalmettoCoverage"] = fnConverter.convertContinuousSetter(&SIGSurface::setPalmettoCoverage);
  fns["SIGSurface.setSecondFuelModelNumber"] = fnConverter.convertContinuousSetter(&SIGSurface::setSecondFuelModelNumber);
  fns["SIGSurface.setSlope"] = fnConverter.convertContinuousSetter(&SIGSurface::setSlope);
  fns["SIGSurface.setSurfaceFireSpreadDirectionMode"] = fnConverter.convertContinuousSetter(&SIGSurface::setSurfaceFireSpreadDirectionMode);
  fns["SIGSurface.setSurfaceRunInDirectionOf"] = fnConverter.convertContinuousSetter(&SIGSurface::setSurfaceRunInDirectionOf);
  fns["SIGSurface.setTwoFuelModelsFirstFuelModelCoverage"] = fnConverter.convertContinuousSetter(&SIGSurface::setTwoFuelModelsFirstFuelModelCoverage);
  fns["SIGSurface.setTwoFuelModelsMethod"] = fnConverter.convertContinuousSetter(&SIGSurface::setTwoFuelModelsMethod);
  fns["SIGSurface.setUserProvidedWindAdjustmentFactor"] = fnConverter.convertContinuousSetter(&SIGSurface::setUserProvidedWindAdjustmentFactor);
  fns["SIGSurface.setWindAdjustmentFactorCalculationMethod"] = fnConverter.convertContinuousSetter(&SIGSurface::setWindAdjustmentFactorCalculationMethod);
  fns["SIGSurface.setWindAndSpreadOrientationMode"] = fnConverter.convertContinuousSetter(&SIGSurface::setWindAndSpreadOrientationMode);
  fns["SIGSurface.setWindDirection"] = fnConverter.convertContinuousSetter(&SIGSurface::setWindDirection);
  fns["SIGSurface.setWindHeightInputMode"] = fnConverter.convertContinuousSetter(&SIGSurface::setWindHeightInputMode);
  fns["SIGSurface.setWindSpeed"] = fnConverter.convertContinuousSetter(&SIGSurface::setWindSpeed);
  fns["SIGSurface.updateSurfaceInputs"] = fnConverter.convertContinuousSetter(&SIGSurface::updateSurfaceInputs);
  fns["SIGSurface.updateSurfaceInputsForPalmettoGallbery"] = fnConverter.convertContinuousSetter(&SIGSurface::updateSurfaceInputsForPalmettoGallbery);
  fns["SIGSurface.updateSurfaceInputsForTwoFuelModels"] = fnConverter.convertContinuousSetter(&SIGSurface::updateSurfaceInputsForTwoFuelModels);
  fns["SIGSurface.updateSurfaceInputsForWesternAspen"] = fnConverter.convertContinuousSetter(&SIGSurface::updateSurfaceInputsForWesternAspen);
  fns["SIGSurface.setFuelModelNumber"] = fnConverter.convertContinuousSetter(&SIGSurface::setFuelModelNumber);

  /// Add Calculate Methods
  fns["doContainRun"] = fnConverter.convertCalculate(&SIGContainAdapter::doContainRun);

  /// Add Getters
  // fns["getHeight"] = fnConverter.convertContinuousGetter(&SIGTestClass::getHeight);
  // fns["getSpeed"] = fnConverter.convertContinuousGetter(&SIGTestClass::getSpeed);
  // fns["getUnits"] = fnConverter.convertDiscreteGetter(&SIGTestClass::getUnits);

  fns["getContainmentStatus"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getContainmentStatus);
  fns["getFinalContainmentArea"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFinalContainmentArea);
  fns["getFinalCost"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFinalCost);
  fns["getFinalFireLineLength"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFinalFireLineLength);
  fns["getFinalFireSize"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFinalFireSize);
  fns["getFinalTimeSinceReport"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFinalTimeSinceReport);
  fns["getFireSizeAtInitialAttack"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getFireSizeAtInitialAttack);
  fns["getPerimeterAtContainment"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getPerimeterAtContainment);
  fns["getPerimeterAtInitialAttack"] = fnConverter.convertContinuousSetter(&SIGContainAdapter::getPerimeterAtInitialAttack);


  fns["getAspenFireSeverity"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenFireSeverity);
  fns["getChaparralFuelType"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralFuelType);
  fns["getMoistureInputMode"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureInputMode);
  fns["getWindAdjustmentFactorCalculationMethod"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindAdjustmentFactorCalculationMethod);
  fns["getWindAndSpreadOrientationMode"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindAndSpreadOrientationMode);
  fns["getWindHeightInputMode"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindHeightInputMode);
  fns["getWindUpslopeAlignmentMode"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindUpslopeAlignmentMode);
  fns["getIsMoistureScenarioDefinedByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getIsMoistureScenarioDefinedByIndex);
  fns["getIsMoistureScenarioDefinedByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getIsMoistureScenarioDefinedByName);
  fns["getIsUsingChaparral"] = fnConverter.convertContinuousGetter(&SIGSurface::getIsUsingChaparral);
  fns["getIsUsingPalmettoGallberry"] = fnConverter.convertContinuousGetter(&SIGSurface::getIsUsingPalmettoGallberry);
  fns["getIsUsingWesternAspen"] = fnConverter.convertContinuousGetter(&SIGSurface::getIsUsingWesternAspen);
  fns["isAllFuelLoadZero"] = fnConverter.convertContinuousGetter(&SIGSurface::isAllFuelLoadZero);
  fns["isFuelDynamic"] = fnConverter.convertContinuousGetter(&SIGSurface::isFuelDynamic);
  fns["isFuelModelDefined"] = fnConverter.convertContinuousGetter(&SIGSurface::isFuelModelDefined);
  fns["isFuelModelReserved"] = fnConverter.convertContinuousGetter(&SIGSurface::isFuelModelReserved);
  fns["isMoistureClassInputNeededForCurrentFuelModel"] = fnConverter.convertContinuousGetter(&SIGSurface::isMoistureClassInputNeededForCurrentFuelModel);
  fns["isUsingTwoFuelModels"] = fnConverter.convertContinuousGetter(&SIGSurface::isUsingTwoFuelModels);
  fns["setMoistureScenarioByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::setMoistureScenarioByIndex);
  fns["setMoistureScenarioByName"] = fnConverter.convertContinuousGetter(&SIGSurface::setMoistureScenarioByName);
  fns["calculateFlameLength"] = fnConverter.convertContinuousGetter(&SIGSurface::calculateFlameLength);
  fns["getAgeOfRough"] = fnConverter.convertContinuousGetter(&SIGSurface::getAgeOfRough);
  fns["getAspect"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspect);
  fns["getAspenCuringLevel"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenCuringLevel);
  fns["getAspenDBH"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenDBH);
  fns["getAspenLoadDeadOneHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenLoadDeadOneHour);
  fns["getAspenLoadDeadTenHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenLoadDeadTenHour);
  fns["getAspenLoadLiveHerbaceous"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenLoadLiveHerbaceous);
  fns["getAspenLoadLiveWoody"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenLoadLiveWoody);
  fns["getAspenSavrDeadOneHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenSavrDeadOneHour);
  fns["getAspenSavrDeadTenHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenSavrDeadTenHour);
  fns["getAspenSavrLiveHerbaceous"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenSavrLiveHerbaceous);
  fns["getAspenSavrLiveWoody"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenSavrLiveWoody);
  fns["getBackingFirelineIntensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getBackingFirelineIntensity);
  fns["getBackingFlameLength"] = fnConverter.convertContinuousGetter(&SIGSurface::getBackingFlameLength);
  fns["getBackingSpreadDistance"] = fnConverter.convertContinuousGetter(&SIGSurface::getBackingSpreadDistance);
  fns["getBackingSpreadRate"] = fnConverter.convertContinuousGetter(&SIGSurface::getBackingSpreadRate);
  fns["getBulkDensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getBulkDensity);
  fns["getCanopyCover"] = fnConverter.convertContinuousGetter(&SIGSurface::getCanopyCover);
  fns["getCanopyHeight"] = fnConverter.convertContinuousGetter(&SIGSurface::getCanopyHeight);
  fns["getChaparralAge"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralAge);
  fns["getChaparralDaysSinceMayFirst"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralDaysSinceMayFirst);
  fns["getChaparralDeadFuelFraction"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralDeadFuelFraction);
  fns["getChaparralDeadMoistureOfExtinction"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralDeadMoistureOfExtinction);
  fns["getChaparralDensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralDensity);
  fns["getChaparralFuelBedDepth"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralFuelBedDepth);
  fns["getChaparralFuelDeadLoadFraction"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralFuelDeadLoadFraction);
  fns["getChaparralHeatOfCombustion"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralHeatOfCombustion);
  fns["getChaparralLiveMoistureOfExtinction"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLiveMoistureOfExtinction);
  fns["getChaparralLoadDeadHalfInchToLessThanOneInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadDeadHalfInchToLessThanOneInch);
  fns["getChaparralLoadDeadLessThanQuarterInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadDeadLessThanQuarterInch);
  fns["getChaparralLoadDeadOneInchToThreeInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadDeadOneInchToThreeInch);
  fns["getChaparralLoadDeadQuarterInchToLessThanHalfInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadDeadQuarterInchToLessThanHalfInch);
  fns["getChaparralLoadLiveHalfInchToLessThanOneInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadLiveHalfInchToLessThanOneInch);
  fns["getChaparralLoadLiveLeaves"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadLiveLeaves);
  fns["getChaparralLoadLiveOneInchToThreeInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadLiveOneInchToThreeInch);
  fns["getChaparralLoadLiveQuarterInchToLessThanHalfInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadLiveQuarterInchToLessThanHalfInch);
  fns["getChaparralLoadLiveStemsLessThanQuaterInch"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralLoadLiveStemsLessThanQuaterInch);
  fns["getChaparralMoisture"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralMoisture);
  fns["getChaparralTotalDeadFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralTotalDeadFuelLoad);
  fns["getChaparralTotalFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralTotalFuelLoad);
  fns["getChaparralTotalLiveFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getChaparralTotalLiveFuelLoad);
  fns["getCharacteristicMoistureByLifeState"] = fnConverter.convertContinuousGetter(&SIGSurface::getCharacteristicMoistureByLifeState);
  fns["getCharacteristicMoistureDead"] = fnConverter.convertContinuousGetter(&SIGSurface::getCharacteristicMoistureDead);
  fns["getCharacteristicMoistureLive"] = fnConverter.convertContinuousGetter(&SIGSurface::getCharacteristicMoistureLive);
  fns["getCharacteristicSAVR"] = fnConverter.convertContinuousGetter(&SIGSurface::getCharacteristicSAVR);
  fns["getCrownRatio"] = fnConverter.convertContinuousGetter(&SIGSurface::getCrownRatio);
  fns["getDirectionOfMaxSpread"] = fnConverter.convertContinuousGetter(&SIGSurface::getDirectionOfMaxSpread);
  fns["getDirectionOfInterest"] = fnConverter.convertContinuousGetter(&SIGSurface::getDirectionOfInterest);
  fns["getElapsedTime"] = fnConverter.convertContinuousGetter(&SIGSurface::getElapsedTime);
  fns["getEllipticalA"] = fnConverter.convertContinuousGetter(&SIGSurface::getEllipticalA);
  fns["getEllipticalB"] = fnConverter.convertContinuousGetter(&SIGSurface::getEllipticalB);
  fns["getEllipticalC"] = fnConverter.convertContinuousGetter(&SIGSurface::getEllipticalC);
  fns["getFireArea"] = fnConverter.convertContinuousGetter(&SIGSurface::getFireArea);
  fns["getFireEccentricity"] = fnConverter.convertContinuousGetter(&SIGSurface::getFireEccentricity);
  fns["getFireLengthToWidthRatio"] = fnConverter.convertContinuousGetter(&SIGSurface::getFireLengthToWidthRatio);
  fns["getFirePerimeter"] = fnConverter.convertContinuousGetter(&SIGSurface::getFirePerimeter);
  fns["getFirelineIntensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getFirelineIntensity);
  fns["getFlameLength"] = fnConverter.convertContinuousGetter(&SIGSurface::getFlameLength);
  fns["getFlankingFirelineIntensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getFlankingFirelineIntensity);
  fns["getFlankingFlameLength"] = fnConverter.convertContinuousGetter(&SIGSurface::getFlankingFlameLength);
  fns["getFlankingSpreadRate"] = fnConverter.convertContinuousGetter(&SIGSurface::getFlankingSpreadRate);
  fns["getFlankingSpreadDistance"] = fnConverter.convertContinuousGetter(&SIGSurface::getFlankingSpreadDistance);
  fns["getFuelHeatOfCombustionDead"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelHeatOfCombustionDead);
  fns["getFuelHeatOfCombustionLive"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelHeatOfCombustionLive);
  fns["getFuelLoadHundredHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelLoadHundredHour);
  fns["getFuelLoadLiveHerbaceous"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelLoadLiveHerbaceous);
  fns["getFuelLoadLiveWoody"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelLoadLiveWoody);
  fns["getFuelLoadOneHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelLoadOneHour);
  fns["getFuelLoadTenHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelLoadTenHour);
  fns["getFuelMoistureOfExtinctionDead"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelMoistureOfExtinctionDead);
  fns["getFuelSavrLiveHerbaceous"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelSavrLiveHerbaceous);
  fns["getFuelSavrLiveWoody"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelSavrLiveWoody);
  fns["getFuelSavrOneHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelSavrOneHour);
  fns["getFuelbedDepth"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelbedDepth);
  fns["getHeadingToBackingRatio"] = fnConverter.convertContinuousGetter(&SIGSurface::getHeadingToBackingRatio);
  fns["getHeatPerUnitArea"] = fnConverter.convertContinuousGetter(&SIGSurface::getHeatPerUnitArea);
  fns["getHeatSink"] = fnConverter.convertContinuousGetter(&SIGSurface::getHeatSink);
  fns["getHeatSource"] = fnConverter.convertContinuousGetter(&SIGSurface::getHeatSource);
  fns["getHeightOfUnderstory"] = fnConverter.convertContinuousGetter(&SIGSurface::getHeightOfUnderstory);
  fns["getLiveFuelMoistureOfExtinction"] = fnConverter.convertContinuousGetter(&SIGSurface::getLiveFuelMoistureOfExtinction);
  fns["getMidflameWindspeed"] = fnConverter.convertContinuousGetter(&SIGSurface::getMidflameWindspeed);
  fns["getMoistureDeadAggregateValue"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureDeadAggregateValue);
  fns["getMoistureHundredHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureHundredHour);
  fns["getMoistureLiveAggregateValue"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureLiveAggregateValue);
  fns["getMoistureLiveHerbaceous"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureLiveHerbaceous);
  fns["getMoistureLiveWoody"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureLiveWoody);
  fns["getMoistureOneHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureOneHour);
  fns["getMoistureScenarioHundredHourByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioHundredHourByIndex);
  fns["getMoistureScenarioHundredHourByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioHundredHourByName);
  fns["getMoistureScenarioLiveHerbaceousByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioLiveHerbaceousByIndex);
  fns["getMoistureScenarioLiveHerbaceousByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioLiveHerbaceousByName);
  fns["getMoistureScenarioLiveWoodyByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioLiveWoodyByIndex);
  fns["getMoistureScenarioLiveWoodyByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioLiveWoodyByName);
  fns["getMoistureScenarioOneHourByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioOneHourByIndex);
  fns["getMoistureScenarioOneHourByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioOneHourByName);
  fns["getMoistureScenarioTenHourByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioTenHourByIndex);
  fns["getMoistureScenarioTenHourByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioTenHourByName);
  fns["getMoistureTenHour"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureTenHour);
  fns["getOverstoryBasalArea"] = fnConverter.convertContinuousGetter(&SIGSurface::getOverstoryBasalArea);
  fns["getPalmettoGallberryCoverage"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberryCoverage);
  fns["getPalmettoGallberryHeatOfCombustionDead"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberryHeatOfCombustionDead);
  fns["getPalmettoGallberryHeatOfCombustionLive"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberryHeatOfCombustionLive);
  fns["getPalmettoGallberryMoistureOfExtinctionDead"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberryMoistureOfExtinctionDead);
  fns["getPalmettoGallberyDeadFineFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyDeadFineFuelLoad);
  fns["getPalmettoGallberyDeadFoliageLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyDeadFoliageLoad);
  fns["getPalmettoGallberyDeadMediumFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyDeadMediumFuelLoad);
  fns["getPalmettoGallberyFuelBedDepth"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyFuelBedDepth);
  fns["getPalmettoGallberyLitterLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyLitterLoad);
  fns["getPalmettoGallberyLiveFineFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyLiveFineFuelLoad);
  fns["getPalmettoGallberyLiveFoliageLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyLiveFoliageLoad);
  fns["getPalmettoGallberyLiveMediumFuelLoad"] = fnConverter.convertContinuousGetter(&SIGSurface::getPalmettoGallberyLiveMediumFuelLoad);
  fns["getReactionIntensity"] = fnConverter.convertContinuousGetter(&SIGSurface::getReactionIntensity);
  fns["getResidenceTime"] = fnConverter.convertContinuousGetter(&SIGSurface::getResidenceTime);
  fns["getSlope"] = fnConverter.convertContinuousGetter(&SIGSurface::getSlope);
  fns["getSlopeFactor"] = fnConverter.convertContinuousGetter(&SIGSurface::getSlopeFactor);
  fns["getSpreadDistance"] = fnConverter.convertContinuousGetter(&SIGSurface::getSpreadDistance);
  fns["getSpreadDistanceInDirectionOfInterest"] = fnConverter.convertContinuousGetter(&SIGSurface::getSpreadDistanceInDirectionOfInterest);
  fns["getSpreadRate"] = fnConverter.convertContinuousGetter(&SIGSurface::getSpreadRate);
  fns["getSpreadRateInDirectionOfInterest"] = fnConverter.convertContinuousGetter(&SIGSurface::getSpreadRateInDirectionOfInterest);
  fns["getSurfaceFireReactionIntensityForLifeState"] = fnConverter.convertContinuousGetter(&SIGSurface::getSurfaceFireReactionIntensityForLifeState);
  fns["getWindDirection"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindDirection);
  fns["getWindSpeed"] = fnConverter.convertContinuousGetter(&SIGSurface::getWindSpeed);
  fns["getAspenFuelModelNumber"] = fnConverter.convertContinuousGetter(&SIGSurface::getAspenFuelModelNumber);
  fns["getFuelModelNumber"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelModelNumber);
  fns["getMoistureScenarioIndexByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioIndexByName);
  fns["getNumberOfMoistureScenarios"] = fnConverter.convertContinuousGetter(&SIGSurface::getNumberOfMoistureScenarios);
  fns["getFuelCode"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelCode);
  fns["getFuelName"] = fnConverter.convertContinuousGetter(&SIGSurface::getFuelName);
  fns["getMoistureScenarioDescriptionByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioDescriptionByIndex);
  fns["getMoistureScenarioDescriptionByName"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioDescriptionByName);
  fns["getMoistureScenarioNameByIndex"] = fnConverter.convertContinuousGetter(&SIGSurface::getMoistureScenarioNameByIndex);

  fns["initializeMembers"] = fnConverter.convertContinuousSetter(&SIGSurface::initializeMembers);
  fns["doSurfaceRun"] = fnConverter.convertContinuousSetter(&SIGSurface::doSurfaceRun);

  // Read JSON file
  std::ifstream file("data.json");
  if (!file) {
    std::cerr << "Failed to open file!" << std::endl;
    return 1;
  }

  json jsonData;
  file >> jsonData;
  file.close();

  json results;

  for (json::iterator it = jsonData.begin(); it != jsonData.end(); ++it) {

    std::string class_name = it.key();
    auto obj = classResolver.resolve("SIGTestClass");

    json inputs = jsonData[class_name]["inputs"];
    json outputs = jsonData[class_name]["outputs"];

    // Apply Inputs
    for (json::iterator it = inputs.begin(); it != inputs.end(); ++it) {

      json input_array = it.value();
      std::string fn_name = it.key();

      if (input_array.size() == 2) {
	double value = input_array[0];
	std::string units = input_array[1];
	std::invoke(std::get<Setter2>(fns[fn_name]), obj, value, units);
      } else if (input_array.size() == 1) {
	std::string value = input_array[0];
	std::invoke(std::get<Setter1>(fns[fn_name]), obj, value);
      }
    }

    // Apply Outputs
    for (json::iterator it = outputs.begin(); it != outputs.end(); ++it) {

      std::string fn_name = it.key();
      json value = it.value();

      if (value.is_string()) {
	std::string units = std::string{value};
	results[class_name][fn_name] = std::invoke(std::get<Getter1>(fns[fn_name]), obj, units);
      } else {
	results[class_name][fn_name] = std::invoke(std::get<Getter0>(fns[fn_name]), obj);
      }
    }
  }

  std::cout << results;

  return 0;
}
