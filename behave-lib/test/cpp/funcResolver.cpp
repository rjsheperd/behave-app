#pragma once

#include <functional>
#include "unitResolver.cpp"
#include "SIGContainAdapter.h"
#include "SIGSurface.h"
#include "SIGSpot.h"
#include "SIGMortality.h"
#include "SIGCrown.h"

// Helper struct to store the number of arguments as a non-type template parameter
template <typename Func>
struct Arity;

// Specialization for non-void member function pointers
template <typename R, typename Class, typename... Args>
struct Arity<R (Class::*)(Args...)> {
    static constexpr std::size_t value = sizeof...(Args);
};

// Specialization for void member function pointers
template <typename Class, typename... Args>
struct Arity<void (Class::*)(Args...)> {
    static constexpr std::size_t value = sizeof...(Args);
};

// Helper function to simplify the usage of getArity
template <typename Func>
constexpr std::size_t getArity(const Func&) {
    return Arity<Func>::value;
}

using Number = std::variant<int, double>;

class FuncConverter {
private:
  UnitResolver _unitResolver;
public:
  FuncConverter(UnitResolver &unitResolver) : _unitResolver(unitResolver) {}

  // Discrete Setter
  template<typename ObjType, typename ArgType>
  void wrapperDiscreteSetter(ObjType* obj, void (ObjType::*func)(ArgType), std::string str) {

    if constexpr (std::is_enum_v<ArgType>) {
      // Convert the input string to the desired ArgType
      ArgType enumValue = static_cast<ArgType>(std::stoi(str));

      // Call the member function of the object with the converted EnumType
      (obj->*func)(enumValue);
    } else {
      // Call the member function of the object with string
      (obj->*func)(str);
    }
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, std::string)> convertDiscreteSetter(void (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      wrapperDiscreteSetter(static_cast<ObjType*>(obj), func, str);
    };
  }

  template<typename ObjType>
  void wrapperContinuousSetter_1(ObjType* obj, void (ObjType::*func)(int), Number number) {

    // Convert the variant
    const int num = std::get<int>(number)

    // Call the member function of the object with the converted variant value
    (obj->*func)(num);
  }

  // Update this function to use `std::variant` instead of `double`
  template<typename ObjType>
  void wrapperContinuousSetter_1(ObjType* obj, void (ObjType::*func)(double), Number number) {

    // Convert the variant
    const double num = std::get<double>(number)

    // Call the member function of the object with the converted variant value
    (obj->*func)(num);
  }

  // Integer variant
  template<typename ObjType>
  std::function<void(void*, Number)> convertContinuousSetter_1(void (ObjType::*func)(int)) {
    return [=](void* obj, Number num) {
      wrapperContinuousSetter_1(static_cast<ObjType*>(obj), func, num);
    };
  }

  // Double variant
  template<typename ObjType>
  std::function<void(void*, Number)> convertContinuousSetter_1(void (ObjType::*func)(double)) {
    return [=](void* obj, Number num) {
      wrapperContinuousSetter_1(static_cast<ObjType*>(obj), func, num);
    };
  }

  // Continous Setter (2-Arity)
  template<typename ObjType, typename EnumType>
  void wrapperContinuousSetter_2(ObjType* obj, void (ObjType::*func)(double, EnumType), double num, std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    (obj->*func)(num, enumValue);
  }

  template<typename ObjType, typename EnumType>
  std::function<void(void*, double, std::string)> convertContinuousSetter_2(void (ObjType::*func)(double, EnumType)) {
    return [=](void* obj, double num, std::string str) {
      wrapperContinuousSetter_2(static_cast<ObjType*>(obj), func, num, str);
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
  double wrapperGetter(ObjType* obj, double (ObjType::*func)(EnumType) const, std::string str) {
    // Convert the input string to the desired EnumType
    EnumType enumValue = _unitResolver.resolveUnit<EnumType>(str);

    // Call the member function of the object with the converted EnumType
    return (obj->*func)(enumValue);
  }

  // Fix this function to allow const functions to be passed in
  template<typename ObjType, typename EnumType>
  std::function<double(void*, std::string)> convertGetter(double (ObjType::*func)(EnumType)) {
    return [=](void* obj, std::string str) {
      return wrapperGetter(static_cast<ObjType*>(obj), func, str);
    };
  }

  template<typename ObjType, typename EnumType>
  std::function<double(void*, std::string)> convertGetter(double (ObjType::*func)(EnumType) const) {
    return [=](void* obj, std::string str) {
      return wrapperGetter(static_cast<ObjType*>(obj), func, str);
    };
  }
};

class FuncResolver {
private:
  std::unordered_map<std::string, std::function<void(void*, std::string)>> discrete_setters;
  std::unordered_map<std::string, std::function<void(void*, double)>> cont_1_setters;
  std::unordered_map<std::string, std::function<void(void*, double, std::string)>> cont_2_setters;
  std::unordered_map<std::string, std::function<double(void*, std::string)>> getters;
  FuncConverter fnConverter;
  
public:

  void addDiscreteSetter(const std::string& name, auto fn) {
    discrete_setters[name] = fnConverter.convertDiscreteSetter(fn);
  }

  void addContinuousSetter_1(const std::string& name, auto fn) {
    cont_1_setters[name] = fnConverter.convertContinuousSetter_1(fn);
  }

  void addContinuousSetter_2(const std::string& name, auto fn) {
    cont_2_setters[name] = fnConverter.convertContinuousSetter_2(fn);
  }

  void addGetter(const std::string& name, auto fn) {
    getters[name] = fnConverter.convertGetter(fn);
  }

  auto discSetter(const std::string& name) {
    return discrete_setters[name];
  }

  std::function<void(void*, double)> contSetter_1(const std::string& name) {
    return cont_1_setters[name];
  }

  std::function<void(void*, double, std::string)> contSetter_2(const std::string& name) {
    return cont_2_setters[name];
  }

  auto getter(const std::string& name) {
    return getters[name];
  }

  FuncResolver(UnitResolver& unitResolver) : fnConverter(unitResolver) {

    // Contain

    addDiscreteSetter("vContainAttackTactic", &SIGContainAdapter::setTactic);
    addGetter("vContainAttackPerimeter", &SIGContainAdapter::getPerimeterAtInitialAttack);
    addGetter("vContainLine", &SIGContainAdapter::getFinalFireLineLength);
    // null("vContainResourceProd", &SIGContainAdapter::addResource);
    addContinuousSetter_2("vContainAttackDist", &SIGContainAdapter::setAttackDistance);
    addContinuousSetter_2("vContainReportSize", &SIGContainAdapter::setReportSize);
    // null("vContainResourceDuration", &SIGContainAdapter::addResource);
    addContinuousSetter_1("vContainReportRatio", &SIGContainAdapter::setLwRatio);
    addGetter("vContainAttackSize", &SIGContainAdapter::getFireSizeAtInitialAttack);
    // null("vContainResourceName", &SIGContainAdapter::addResource);
    addGetter("vContainSize", &SIGContainAdapter::getFinalContainmentArea);
    // null("vContainResourceArrival", &SIGContainAdapter::addResource);
    addGetter("vContainTime", &SIGContainAdapter::getFinalTimeSinceReport);
    addContinuousSetter_2("vContainReportSpread", &SIGContainAdapter::setReportRate);

    // Crown
    //addDiscreteSetter("vWindAdjustmentFactorCalculationMethod", &SIGCrown::setWindAdjustmentFactorCalculationMethod);
    //addGetter("vCrownFireCritSurfFireInt", &SIGCrown::getCrownCriticalSurfaceFirelineIntensity);
    //addDiscreteSetter("vCalculateCrownFireUsing", &SIGCrown::setCrownFireCalculationMethod);
    //addGetter("vSurfaceFireSpreadAtVector", &SIGCrown::getCrownFireSpreadRate);
    //addGetter("vCrownFireActiveFireLineInt", &SIGCrown::getCrownFirelineIntensity);
    //addContinuousSetter_2("vSurfaceFuelMoisDead10", &SIGCrown::setMoistureTenHour);
    //addContinuousSetter_2("vTreeCoverHt", &SIGCrown::setCanopyHeight);
    //addContinuousSetter_1("vTreeCrownRatio", &SIGCrown::setCrownRatio);
    //addGetter("vCrownFireCritSurfSpreadRate", &SIGCrown::getCrownCriticalFireSpreadRate);
    //addContinuousSetter_2("vTreeFoliarMois", &SIGCrown::setMoistureFoliar);
    //addContinuousSetter_2("vSurfaceFuelMoisLiveWood", &SIGCrown::setMoistureLiveWoody);
    //addGetter("vSurfaceFireDistAtVector", &SIGCrown::getCrownFireSpreadDistance);
    //addGetter("vSurfaceFireFlameLengAtHead", &SIGCrown::getCrownFlameLength);
    //addContinuousSetter_2("vSurfaceFuelMoisDead100", &SIGCrown::setMoistureHundredHour);
    //addGetter("vCrownFireCritSurfFlameLeng", &SIGCrown::getCrownCriticalSurfaceFlameLength);
    //addContinuousSetter_1("vSurfaceFuelBedModelNumber", &SIGCrown::setFuelModelNumber);
    //addContinuousSetter_2("vSurfaceFuelMoisLiveHerb", &SIGCrown::setMoistureLiveHerbaceous);
    //addDiscreteSetter("vMoistureInputMode", &SIGCrown::setMoistureInputMode);
    //addContinuousSetter_2("vTreeCrownBaseHt", &SIGCrown::setCanopyBaseHeight);
    //addGetter("vSurfaceFireArea", &SIGCrown::getCrownFireArea);
    //addGetter("vCrownFireCritCrownSpreadRate", &SIGCrown::getCrownCriticalFireSpreadRate);
    //addContinuousSetter_2("vWindSpeedAt20FtUpslope", &SIGCrown::setWindSpeed);
    //addContinuousSetter_1("vWindDirFromNorth", &SIGCrown::setWindDirection);
    //addContinuousSetter_2("vSurfaceFuelMoisDead1", &SIGCrown::setMoistureOneHour);
    //addGetter("vSurfaceFirePerimeter", &SIGCrown::getCrownFirePerimeter);
    //addContinuousSetter_1("vSiteAspectDirFromNorth", &SIGCrown::setAspect);
    //addContinuousSetter_2("vTreeCanopyCover", &SIGCrown::setCanopyCover);
    //addDiscreteSetter("vWindAndSpreadDirections", &SIGCrown::setWindAndSpreadOrientationMode);
    //addContinuousSetter_2("vTreeCanopyBulkDens", &SIGCrown::setCanopyBulkDensity);
    //addContinuousSetter_1("vWindAdjFactor", &SIGCrown::setUserProvidedWindAdjustmentFactor);
    //addDiscreteSetter("vWindMeasuredAt", &SIGCrown::setWindHeightInputMode);
    //addContinuousSetter_2("vSiteSlopeFraction", &SIGCrown::setSlope);

    //// Mortality
    //addDiscreteSetter("vTreeSpecies", &SIGMortality::setSpeciesCode);
    //addDiscreteSetter("vScorchHeightOrFlameLength", &SIGMortality::setFlameLengthOrScorchHeightSwitch);
    //addContinuousSetter_2("vTreeCoverHt", &SIGMortality::setTreeHeight);
    //addContinuousSetter_2("vBoleCharHeight", &SIGMortality::setBoleCharHeight);
    //addGetter("vSurfaceFireScorchHtAtVector", &SIGMortality::getCalculatedScorchHeight);
    //addContinuousSetter_1("vTreeCrownRatio", &SIGMortality::setCrownRatio);
    //addGetter("vTreeBarkThickness", &SIGMortality::getBarkThickness);
    //addGetter("vTreeMortalityRateAtVector", &SIGMortality::getProbabilityOfMortality);
    //addContinuousSetter_1("vCambiumKillRating", &SIGMortality::setCambiumKillRating);
    //addGetter("vTreeCrownLengScorchedAtVector", &SIGMortality::getTreeCrownLengthScorched);
    //addContinuousSetter_2("vSurfaceFireFlameLengAtVector", &SIGMortality::setSurfaceFireFlameLength);
    //addDiscreteSetter("vTreeSpeciesMortality", &SIGMortality::setEquationType);
    //addContinuousSetter_1("vCrownDamage", &SIGMortality::setCrownDamage);
    //addContinuousSetter_2("vSurfaceFireScorchHtAtVector", &SIGMortality::setSurfaceFireScorchHeight);
    //addContinuousSetter_2("vCrownFireCritSurfFireInt", &SIGMortality::setFirelineIntensity);
    //addContinuousSetter_2("vSurfaceFireLineIntAtHead", &SIGMortality::setFirelineIntensity);
    //addGetter("vTreeCrownVolScorchedAtVector", &SIGMortality::getTreeCrownVolumeScorched);
    //addDiscreteSetter("vRegionCode", &SIGMortality::setRegion);
    //addContinuousSetter_2("vWindSpeedAtMidflameUpslope", &SIGMortality::setMidFlameWindSpeed);
    //addDiscreteSetter("vBeetleDamage", &SIGMortality::setBeetleDamage);
    //addContinuousSetter_2("vTreeDbh", &SIGMortality::setDBH);
    //addContinuousSetter_2("vTreeCount", &SIGMortality::setTreeDensityPerUnitArea);
    //addContinuousSetter_2("vWthrAirTemp", &SIGMortality::setAirTemperature);

    //// Spot
    //addContinuousSetter_1("vSpotTorchingTrees", &SIGSpot::setTorchingTrees);
    //addContinuousSetter_2("vTreeCoverHtDownwind", &SIGSpot::setDownwindCoverHeight);
    //addContinuousSetter_2("vSiteRidgeToValleyDist", &SIGSpot::setRidgeToValleyDistance);
    //addContinuousSetter_2("vTreeHt", &SIGSpot::setTreeHeight);
    //addDiscreteSetter("vTreeCanopyCoverDownwind", &SIGSpot::setDownwindCanopyMode);
    //addContinuousSetter_2("vTreeCoverHt", &SIGSpot::setTreeHeight);
    //addContinuousSetter_2("vSiteRidgeToValleyElev", &SIGSpot::setRidgeToValleyElevation);
    //addGetter("vSpotDistActiveCrown", &SIGSpot::getMaxMountainousTerrainSpottingDistanceFromTorchingTrees);
    //addGetter("vSpotDistBurningPile", &SIGSpot::getMaxMountainousTerrainSpottingDistanceFromBurningPile);
    //addGetter("vSpotFlameHtActiveCrown", &SIGSpot::getFlameHeightForTorchingTrees);
    //addDiscreteSetter("vTreeSpeciesSpot", &SIGSpot::setTreeSpecies);
    //addGetter("vSpotDistTorchingTrees", &SIGSpot::getMaxMountainousTerrainSpottingDistanceFromTorchingTrees);
    //addContinuousSetter_2("vCrownFireActiveFlameLeng", &SIGSpot::setFlameLength);
    //addGetter("vSpotFirebrandHtBurningPile", &SIGSpot::getMaxFirebrandHeightFromBurningPile);
    //addDiscreteSetter("vSpotFireSource", &SIGSpot::setLocation);
    //addGetter("vSpotDistSurfaceFire", &SIGSpot::getMaxMountainousTerrainSpottingDistanceFromSurfaceFire);
    //addContinuousSetter_2("vWindSpeedAt20FtUpslope", &SIGSpot::setWindSpeedAtTwentyFeet);
    //addContinuousSetter_2("vTreeDbh", &SIGSpot::setDBH);

    //// Surface
    //addGetter("vSurfaceFuelPalmettoLoadLive10", &SIGSurface::getPalmettoGallberyDeadMediumFuelLoad);
    //addGetter("vSurfaceFuelBedBulkDensity", &SIGSurface::getBulkDensity);
    //addDiscreteSetter("vMoistureInputMode", &SIGSurface::setMoistureInputMode);
    //addDiscreteSetter("vMultipleFuelModels", &SIGSurface::setTwoFuelModelsMethod);
    //addGetter("vSurfaceFuelPalmettoLoadLive10", &SIGSurface::getPalmettoGallberyLiveMediumFuelLoad);
    //addDiscreteSetter("vSurfaceSpreadDirectionMode", &SIGSurface::setSurfaceFireSpreadDirectionMode);
    //addGetter("vSurfaceFireArea", &SIGSurface::getFireArea);
    //addGetter("vSurfaceFuelPalmettoLoadLive1", &SIGSurface::getPalmettoGallberyLiveFineFuelLoad);
    //addGetter("vSurfaceFuelBedHeatSink", &SIGSurface::getHeatSink);
    //addGetter("vSurfaceFuelChaparralLoadDead2", &SIGSurface::getChaparralLoadDeadQuarterInchToLessThanHalfInch);
    //addContinuousSetter_2("vSurfaceFuelMoisDead10", &SIGSurface::setMoistureTenHour);
    //addGetter("vSurfaceFuelChaparralLoadLive3", &SIGSurface::getChaparralLoadLiveHalfInchToLessThanOneInch);
    //addContinuousSetter_2("vWindSpeedAt20FtUpslope", &SIGSurface::setWindSpeed);
    //addGetter("vSurfaceFireReactionIntLive", &SIGSurface::getSurfaceFireReactionIntensityForLifeState);
    //addGetter("vSurfaceFireResidenceTime", &SIGSurface::getResidenceTime);
    //addContinuousSetter_2("vSurfaceFireElapsedTime", &SIGSurface::setElapsedTime);
    //addContinuousSetter_2("vSiteSlopeFraction", &SIGSurface::setSlope);
    //addGetter("vSurfaceFuelChaparralLoadLive2", &SIGSurface::getChaparralLoadLiveQuarterInchToLessThanHalfInch);
    //addDiscreteSetter("vSurfaceFuelAspenType", &SIGSurface::setAspenFuelModelNumber);
    //addGetter("vSurfaceFireSpreadAtBack", &SIGSurface::getBackingSpreadRate);
    //addGetter("vSurfaceFuelAspenSavrDead1", &SIGSurface::getAspenSavrDeadOneHour);
    //addContinuousSetter_1("vSiteAspectDirFromNorth", &SIGSurface::setAspect);
    //addContinuousSetter_1("vWindAdjFactor", &SIGSurface::setUserProvidedWindAdjustmentFactor);
    //// null("vSurfaceFuelBedMoisDead", &SIGSurface::getCharacteristicMoistureByLifeState);
    //addDiscreteSetter("vWindAdjustmentFactorCalculationMethod", &SIGSurface::setWindAdjustmentFactorCalculationMethod);
    //// null("vSurfaceFuelBedMoisLive", &SIGSurface::getCharacteristicMoistureByLifeState);
    //addGetter("vSurfaceFuelChaparralLoadLive4", &SIGSurface::getChaparralLoadLiveOneInchToThreeInch);
    //addDiscreteSetter("vSurfaceFuelBedModelCode", &SIGSurface::setFuelModelNumber);
    //addGetter("vSurfaceFireLineIntAtHead", &SIGSurface::getBackingFirelineIntensity);
    //addGetter("vSurfaceFireFlameLengAtHead", &SIGSurface::getFlameLength);
    //addGetter("vSurfaceFireLineIntAtHead", &SIGSurface::getFlankingFirelineIntensity);
    //addContinuousSetter_1("vTreeCrownRatio", &SIGSurface::setCrownRatio);
    //addContinuousSetter_2("vSurfaceFuelPalmettoHeight", &SIGSurface::setHeightOfUnderstory);
    //addDiscreteSetter("vSurfaceFuelChaparralType", &SIGSurface::setChaparralFuelType);
    //addGetter("vSurfaceFuelAspenLoadDead1", &SIGSurface::getAspenLoadDeadOneHour);
    //addGetter("vSurfaceFireFlameLengAtHead", &SIGSurface::getFlankingFlameLength);
    //addGetter("vSurfaceFuelChaparralLoadLiveLeaf", &SIGSurface::getChaparralLoadLiveLeaves);
    //addGetter("vSurfaceFireHeatPerUnitArea", &SIGSurface::getHeatPerUnitArea);
    //addContinuousSetter_2("vSurfaceFuelMoisDead100", &SIGSurface::setMoistureHundredHour);
    //addGetter("vSurfaceFuelLoadLive", &SIGSurface::getChaparralTotalLiveFuelLoad);
    //addGetter("vSurfaceFuelChaparralLoadDead3", &SIGSurface::getChaparralLoadDeadHalfInchToLessThanOneInch);
    //addDiscreteSetter("vWindMeasuredAt", &SIGSurface::setWindHeightInputMode);
    //addContinuousSetter_2("vSurfaceFuelPalmettoOverstoryBasalArea", &SIGSurface::setOverstoryBasalArea);
    //addContinuousSetter_2("vSurfaceFuelBedDepth", &SIGSurface::setChaparralFuelBedDepth);
    //addGetter("vSurfaceFireHeatSource", &SIGSurface::getHeatSource);
    //addGetter("vSurfaceFuelPalmettoLoadLiveFoliage", &SIGSurface::getPalmettoGallberyLiveFoliageLoad);
    //addContinuousSetter_1("vDirectionOfInterest", &SIGSurface::setDirectionOfInterest);
    //addGetter("vSurfaceFuelChaparralLoadDead4", &SIGSurface::getChaparralLoadDeadOneInchToThreeInch);
    //addGetter("vSurfaceFuelChaparralLoadLiveLeaf", &SIGSurface::getPalmettoGallberyLitterLoad);
    //addGetter("vSurfaceFireLineIntAtHead", &SIGSurface::getFirelineIntensity);
    //addGetter("vSurfaceFireReactionInt", &SIGSurface::getReactionIntensity);
    //addDiscreteSetter("vSurfaceRunInDirectionOf", &SIGSurface::setSurfaceRunInDirectionOf);
    //addGetter("vSurfaceFireDistAtBack", &SIGSurface::getBackingSpreadDistance);
    //addContinuousSetter_2("vSurfaceFuelMoisLiveHerb", &SIGSurface::setMoistureLiveHerbaceous);
    //addGetter("vSurfaceFuelBedSigma", &SIGSurface::getCharacteristicSAVR);
    //addContinuousSetter_2("vTreeCoverHt", &SIGSurface::setCanopyHeight);
    //addGetter("vSurfaceFireSpreadAtVector", &SIGSurface::getSpreadRate);
    //addGetter("vSurfaceFuelBedMextLive", &SIGSurface::getLiveFuelMoistureOfExtinction);
    //addContinuousSetter_1("vSurfaceFuelChaparralDeadFuelFraction", &SIGSurface::setChaparralFuelDeadLoadFraction);
    //addContinuousSetter_2("vSurfaceFuelPalmettoCover", &SIGSurface::setPalmettoCoverage);
    //addGetter("vSurfaceFuelAspenSavrLiveWoody", &SIGSurface::getAspenSavrLiveWoody);
    //addContinuousSetter_1("vSurfaceFuelPalmettoAge", &SIGSurface::setAgeOfRough);
    //addContinuousSetter_2("vSurfaceFuelMoisDead1", &SIGSurface::setMoistureOneHour);
    //addGetter("vSurfaceFireDistAtHead", &SIGSurface::getSpreadDistance);
    //addGetter("vSurfaceFuelPalmettoLoadLive1", &SIGSurface::getPalmettoGallberyDeadFineFuelLoad);
    //addGetter("vSurfaceFireDistAtFlank", &SIGSurface::getFlankingSpreadDistance);
    //addGetter("vSurfaceFuelPalmettoLoadLiveFoliage", &SIGSurface::getPalmettoGallberyDeadFoliageLoad);
    //addGetter("vSurfaceFuelBedDepth", &SIGSurface::getPalmettoGallberyFuelBedDepth);
    //addGetter("vSurfaceFuelAspenLoadLiveWoody", &SIGSurface::getAspenLoadDeadOneHour);
    //addGetter("vSurfaceFuelChaparralLoadDead1", &SIGSurface::getChaparralLoadDeadLessThanQuarterInch);
    //addContinuousSetter_1("vWindDirFromNorth", &SIGSurface::setWindDirection);
    //addGetter("vSurfaceFireReactionIntDead", &SIGSurface::getSurfaceFireReactionIntensityForLifeState);
    //addGetter("vSurfaceFuelLoadDead", &SIGSurface::getChaparralTotalDeadFuelLoad);
    //addGetter("vSurfaceFireFlameLengAtHead", &SIGSurface::getBackingFlameLength);
    //addContinuousSetter_2("vTreeCanopyCover", &SIGSurface::setCanopyCover);
    //addDiscreteSetter("vChaparralFuelLoadInputMode", &SIGSurface::setChaparralFuelLoadInputMode);
    //addGetter("vSurfaceFuelAspenLoadLiveHerb", &SIGSurface::getAspenLoadLiveHerbaceous);
    //addContinuousSetter_2("vSurfaceFuelChaparralLoadTotal", &SIGSurface::setChaparralTotalFuelLoad);
    //addGetter("vSurfaceFuelChaparralLoadLive1", &SIGSurface::getChaparralLoadLiveStemsLessThanQuaterInch);
    //addGetter("vSurfaceFireSpreadAtFlank", &SIGSurface::getFlankingSpreadRate);
    //addDiscreteSetter("vWindAndSpreadDirections", &SIGSurface::setWindAndSpreadOrientationMode);
    //addGetter("vSurfaceFirePerimeter", &SIGSurface::getFirePerimeter);
    //addContinuousSetter_2("vSurfaceFuelMoisLiveWood", &SIGSurface::setMoistureLiveWoody);
  };

};
