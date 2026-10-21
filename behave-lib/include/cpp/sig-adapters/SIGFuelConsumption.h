//------------------------------------------------------------------------------
/*! \file SIGFuelConsumption.h
    \brief behave-lib adapter for FuelConsumption (FOFEM fuel consumption, Burnup and emissions): the inputs' setters and
    getters as methods of the module, so the app can call them on one object,
    and the message as char* for WebIDL.
*/
#pragma once
#include "fuelConsumption.h"
#include "SIGString.h"

class SIGFuelConsumption : public FuelConsumption {
public:
  SIGFuelConsumption();
  SIGFuelConsumption(const SIGFuelConsumption& rhs);
  SIGFuelConsumption& operator=(const SIGFuelConsumption& rhs);

  void initializeMembers();

  /*! \brief Why the last run failed (FuelConsumption::getErrorMessage), as char*. */
  char* getErrorMessage() const;

  // Inputs: forwarded to FuelConsumptionInputs
  void setFuelLoadLitter(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadOneHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadTenHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadHundredHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadSoundThreeToSixInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadSoundSixToNineInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadSoundNineToTwentyInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadSoundTwentyInchAndLarger(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadRottenThreeToSixInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadRottenSixToNineInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadRottenNineToTwentyInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadRottenTwentyInchAndLarger(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void distributeThousandHourFuelLoad(double totalFuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits, double rottenFraction, FractionUnits::FractionUnitsEnum fractionUnits, ThousandHourLoadDistribution::ThousandHourLoadDistributionEnum distribution);
  void setFuelLoadDuff(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setDuffDepth(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits);
  void setFuelLoadHerbaceous(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadShrub(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadCrownFoliage(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setFuelLoadCrownBranch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits);
  void setPercentOfCrownBurned(double percentOfCrownBurned, FractionUnits::FractionUnitsEnum fractionUnits);
  void setMoistureDuff(double moisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setMoistureTenHour(double moisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setMoistureThousandHour(double moisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setMoistureLitter(double moisture, FractionUnits::FractionUnitsEnum moistureUnits);
  void setDuffMoistureMethod(DuffMoistureMethod::DuffMoistureMethodEnum duffMoistureMethod);
  void setRegion(FuelConsumptionRegion::FuelConsumptionRegionEnum region);
  void setCoverGroup(CoverGroup::CoverGroupEnum coverGroup);
  void setBurnSeason(BurnSeason::BurnSeasonEnum burnSeason);
  void setFuelCategory(FuelCategory::FuelCategoryEnum fuelCategory);
  void setDuffType(DuffType::DuffTypeEnum duffType);
  void setUseConsumptionEquations(bool useConsumptionEquations);
  void setLitterConsumptionEquation(LitterConsumptionEquation::LitterConsumptionEquationEnum equation);
  void setDuffConsumptionEquation(DuffConsumptionEquation::DuffConsumptionEquationEnum equation);
  void setDuffDepthReductionEquation(DuffConsumptionEquation::DuffConsumptionEquationEnum equation);
  void setMineralSoilExposureEquation(MineralSoilExposureEquation::MineralSoilExposureEquationEnum equation);
  void setHerbaceousConsumptionEquation(HerbaceousConsumptionEquation::HerbaceousConsumptionEquationEnum equation);
  void setShrubConsumptionEquation(ShrubConsumptionEquation::ShrubConsumptionEquationEnum equation);
  bool setEmissionFactorGroups(int flamingGroupNumber, int coarseWoodyGroupNumber, int duffGroupNumber);
  void setUseLegacyEmissionFactors(bool useLegacyEmissionFactors);
  double getFuelLoadLitter(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadOneHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadTenHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadHundredHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadSoundThreeToSixInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadSoundSixToNineInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadSoundNineToTwentyInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadSoundTwentyInchAndLarger(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadRottenThreeToSixInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadRottenSixToNineInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadRottenNineToTwentyInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadRottenTwentyInchAndLarger(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadThousandHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadDuff(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getDuffDepth(LengthUnits::LengthUnitsEnum lengthUnits) const;
  double getFuelLoadHerbaceous(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadShrub(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadCrownFoliage(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getFuelLoadCrownBranch(LoadingUnits::LoadingUnitsEnum loadingUnits) const;
  double getPercentOfCrownBurned(FractionUnits::FractionUnitsEnum fractionUnits) const;
  double getMoistureDuff(FractionUnits::FractionUnitsEnum moistureUnits) const;
  double getMoistureTenHour(FractionUnits::FractionUnitsEnum moistureUnits) const;
  double getMoistureThousandHour(FractionUnits::FractionUnitsEnum moistureUnits) const;
  double getMoistureLitter(FractionUnits::FractionUnitsEnum moistureUnits) const;
  DuffMoistureMethod::DuffMoistureMethodEnum getDuffMoistureMethod() const;
  FuelConsumptionRegion::FuelConsumptionRegionEnum getRegion() const;
  CoverGroup::CoverGroupEnum getCoverGroup() const;
  BurnSeason::BurnSeasonEnum getBurnSeason() const;
  FuelCategory::FuelCategoryEnum getFuelCategory() const;
  DuffType::DuffTypeEnum getDuffType() const;
  bool getUseConsumptionEquations() const;
  LitterConsumptionEquation::LitterConsumptionEquationEnum getLitterConsumptionEquation() const;
  DuffConsumptionEquation::DuffConsumptionEquationEnum getDuffConsumptionEquation() const;
  DuffConsumptionEquation::DuffConsumptionEquationEnum getDuffDepthReductionEquation() const;
  MineralSoilExposureEquation::MineralSoilExposureEquationEnum getMineralSoilExposureEquation() const;
  HerbaceousConsumptionEquation::HerbaceousConsumptionEquationEnum getHerbaceousConsumptionEquation() const;
  ShrubConsumptionEquation::ShrubConsumptionEquationEnum getShrubConsumptionEquation() const;
  int getFlamingEmissionFactorGroup() const;
  int getCoarseWoodyEmissionFactorGroup() const;
  int getDuffEmissionFactorGroup() const;
  bool getUseLegacyEmissionFactors() const;
};
