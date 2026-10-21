//------------------------------------------------------------------------------
/*! \file SIGFuelConsumption.cpp
    \brief See SIGFuelConsumption.h.
*/
#include "SIGFuelConsumption.h"

SIGFuelConsumption::SIGFuelConsumption()
  : FuelConsumption()
{
}

SIGFuelConsumption::SIGFuelConsumption(const SIGFuelConsumption& rhs)
  : FuelConsumption(rhs)
{
}

SIGFuelConsumption& SIGFuelConsumption::operator=(const SIGFuelConsumption& rhs)
{
  FuelConsumption::operator=(rhs);
  return *this;
}

void SIGFuelConsumption::initializeMembers()
{
  FuelConsumption::initializeMembers();
}

char* SIGFuelConsumption::getErrorMessage() const
{
  return SIGString::str2charptr(FuelConsumption::getErrorMessage());
}


void SIGFuelConsumption::setFuelLoadLitter(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadLitter(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadOneHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadOneHour(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadTenHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadTenHour(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadHundredHour(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadHundredHour(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadSoundThreeToSixInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadSoundThreeToSixInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadSoundSixToNineInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadSoundSixToNineInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadSoundNineToTwentyInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadSoundNineToTwentyInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadSoundTwentyInchAndLarger(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadSoundTwentyInchAndLarger(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadRottenThreeToSixInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadRottenThreeToSixInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadRottenSixToNineInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadRottenSixToNineInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadRottenNineToTwentyInch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadRottenNineToTwentyInch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadRottenTwentyInchAndLarger(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadRottenTwentyInchAndLarger(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::distributeThousandHourFuelLoad(double totalFuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits, double rottenFraction, FractionUnits::FractionUnitsEnum fractionUnits, ThousandHourLoadDistribution::ThousandHourLoadDistributionEnum distribution)
{
  getInputs().distributeThousandHourFuelLoad(totalFuelLoad, loadingUnits, rottenFraction, fractionUnits, distribution);
}

void SIGFuelConsumption::setFuelLoadDuff(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadDuff(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setDuffDepth(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits)
{
  getInputs().setDuffDepth(duffDepth, lengthUnits);
}

void SIGFuelConsumption::setFuelLoadHerbaceous(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadHerbaceous(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadShrub(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadShrub(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadCrownFoliage(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadCrownFoliage(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setFuelLoadCrownBranch(double fuelLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setFuelLoadCrownBranch(fuelLoad, loadingUnits);
}

void SIGFuelConsumption::setPercentOfCrownBurned(double percentOfCrownBurned, FractionUnits::FractionUnitsEnum fractionUnits)
{
  getInputs().setPercentOfCrownBurned(percentOfCrownBurned, fractionUnits);
}

void SIGFuelConsumption::setMoistureDuff(double moisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setMoistureDuff(moisture, moistureUnits);
}

void SIGFuelConsumption::setMoistureTenHour(double moisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setMoistureTenHour(moisture, moistureUnits);
}

void SIGFuelConsumption::setMoistureThousandHour(double moisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setMoistureThousandHour(moisture, moistureUnits);
}

void SIGFuelConsumption::setMoistureLitter(double moisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setMoistureLitter(moisture, moistureUnits);
}

void SIGFuelConsumption::setDuffMoistureMethod(DuffMoistureMethod::DuffMoistureMethodEnum duffMoistureMethod)
{
  getInputs().setDuffMoistureMethod(duffMoistureMethod);
}

void SIGFuelConsumption::setRegion(FuelConsumptionRegion::FuelConsumptionRegionEnum region)
{
  getInputs().setRegion(region);
}

void SIGFuelConsumption::setCoverGroup(CoverGroup::CoverGroupEnum coverGroup)
{
  getInputs().setCoverGroup(coverGroup);
}

void SIGFuelConsumption::setBurnSeason(BurnSeason::BurnSeasonEnum burnSeason)
{
  getInputs().setBurnSeason(burnSeason);
}

void SIGFuelConsumption::setFuelCategory(FuelCategory::FuelCategoryEnum fuelCategory)
{
  getInputs().setFuelCategory(fuelCategory);
}

void SIGFuelConsumption::setDuffType(DuffType::DuffTypeEnum duffType)
{
  getInputs().setDuffType(duffType);
}

void SIGFuelConsumption::setUseConsumptionEquations(bool useConsumptionEquations)
{
  getInputs().setUseConsumptionEquations(useConsumptionEquations);
}

void SIGFuelConsumption::setLitterConsumptionEquation(LitterConsumptionEquation::LitterConsumptionEquationEnum equation)
{
  getInputs().setLitterConsumptionEquation(equation);
}

void SIGFuelConsumption::setDuffConsumptionEquation(DuffConsumptionEquation::DuffConsumptionEquationEnum equation)
{
  getInputs().setDuffConsumptionEquation(equation);
}

void SIGFuelConsumption::setDuffDepthReductionEquation(DuffConsumptionEquation::DuffConsumptionEquationEnum equation)
{
  getInputs().setDuffDepthReductionEquation(equation);
}

void SIGFuelConsumption::setMineralSoilExposureEquation(MineralSoilExposureEquation::MineralSoilExposureEquationEnum equation)
{
  getInputs().setMineralSoilExposureEquation(equation);
}

void SIGFuelConsumption::setHerbaceousConsumptionEquation(HerbaceousConsumptionEquation::HerbaceousConsumptionEquationEnum equation)
{
  getInputs().setHerbaceousConsumptionEquation(equation);
}

void SIGFuelConsumption::setShrubConsumptionEquation(ShrubConsumptionEquation::ShrubConsumptionEquationEnum equation)
{
  getInputs().setShrubConsumptionEquation(equation);
}

bool SIGFuelConsumption::setEmissionFactorGroups(int flamingGroupNumber, int coarseWoodyGroupNumber, int duffGroupNumber)
{
  return getInputs().setEmissionFactorGroups(flamingGroupNumber, coarseWoodyGroupNumber, duffGroupNumber);
}

void SIGFuelConsumption::setUseLegacyEmissionFactors(bool useLegacyEmissionFactors)
{
  getInputs().setUseLegacyEmissionFactors(useLegacyEmissionFactors);
}

double SIGFuelConsumption::getFuelLoadLitter(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadLitter(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadOneHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadOneHour(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadTenHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadTenHour(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadHundredHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadHundredHour(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadSoundThreeToSixInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadSoundThreeToSixInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadSoundSixToNineInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadSoundSixToNineInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadSoundNineToTwentyInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadSoundNineToTwentyInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadSoundTwentyInchAndLarger(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadSoundTwentyInchAndLarger(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadRottenThreeToSixInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadRottenThreeToSixInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadRottenSixToNineInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadRottenSixToNineInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadRottenNineToTwentyInch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadRottenNineToTwentyInch(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadRottenTwentyInchAndLarger(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadRottenTwentyInchAndLarger(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadThousandHour(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadThousandHour(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadDuff(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadDuff(loadingUnits);
}

double SIGFuelConsumption::getDuffDepth(LengthUnits::LengthUnitsEnum lengthUnits) const
{
  return getInputs().getDuffDepth(lengthUnits);
}

double SIGFuelConsumption::getFuelLoadHerbaceous(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadHerbaceous(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadShrub(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadShrub(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadCrownFoliage(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadCrownFoliage(loadingUnits);
}

double SIGFuelConsumption::getFuelLoadCrownBranch(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getFuelLoadCrownBranch(loadingUnits);
}

double SIGFuelConsumption::getPercentOfCrownBurned(FractionUnits::FractionUnitsEnum fractionUnits) const
{
  return getInputs().getPercentOfCrownBurned(fractionUnits);
}

double SIGFuelConsumption::getMoistureDuff(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getMoistureDuff(moistureUnits);
}

double SIGFuelConsumption::getMoistureTenHour(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getMoistureTenHour(moistureUnits);
}

double SIGFuelConsumption::getMoistureThousandHour(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getMoistureThousandHour(moistureUnits);
}

double SIGFuelConsumption::getMoistureLitter(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getMoistureLitter(moistureUnits);
}

DuffMoistureMethod::DuffMoistureMethodEnum SIGFuelConsumption::getDuffMoistureMethod() const
{
  return getInputs().getDuffMoistureMethod();
}

FuelConsumptionRegion::FuelConsumptionRegionEnum SIGFuelConsumption::getRegion() const
{
  return getInputs().getRegion();
}

CoverGroup::CoverGroupEnum SIGFuelConsumption::getCoverGroup() const
{
  return getInputs().getCoverGroup();
}

BurnSeason::BurnSeasonEnum SIGFuelConsumption::getBurnSeason() const
{
  return getInputs().getBurnSeason();
}

FuelCategory::FuelCategoryEnum SIGFuelConsumption::getFuelCategory() const
{
  return getInputs().getFuelCategory();
}

DuffType::DuffTypeEnum SIGFuelConsumption::getDuffType() const
{
  return getInputs().getDuffType();
}

bool SIGFuelConsumption::getUseConsumptionEquations() const
{
  return getInputs().getUseConsumptionEquations();
}

LitterConsumptionEquation::LitterConsumptionEquationEnum SIGFuelConsumption::getLitterConsumptionEquation() const
{
  return getInputs().getLitterConsumptionEquation();
}

DuffConsumptionEquation::DuffConsumptionEquationEnum SIGFuelConsumption::getDuffConsumptionEquation() const
{
  return getInputs().getDuffConsumptionEquation();
}

DuffConsumptionEquation::DuffConsumptionEquationEnum SIGFuelConsumption::getDuffDepthReductionEquation() const
{
  return getInputs().getDuffDepthReductionEquation();
}

MineralSoilExposureEquation::MineralSoilExposureEquationEnum SIGFuelConsumption::getMineralSoilExposureEquation() const
{
  return getInputs().getMineralSoilExposureEquation();
}

HerbaceousConsumptionEquation::HerbaceousConsumptionEquationEnum SIGFuelConsumption::getHerbaceousConsumptionEquation() const
{
  return getInputs().getHerbaceousConsumptionEquation();
}

ShrubConsumptionEquation::ShrubConsumptionEquationEnum SIGFuelConsumption::getShrubConsumptionEquation() const
{
  return getInputs().getShrubConsumptionEquation();
}

int SIGFuelConsumption::getFlamingEmissionFactorGroup() const
{
  return getInputs().getFlamingEmissionFactorGroup();
}

int SIGFuelConsumption::getCoarseWoodyEmissionFactorGroup() const
{
  return getInputs().getCoarseWoodyEmissionFactorGroup();
}

int SIGFuelConsumption::getDuffEmissionFactorGroup() const
{
  return getInputs().getDuffEmissionFactorGroup();
}

bool SIGFuelConsumption::getUseLegacyEmissionFactors() const
{
  return getInputs().getUseLegacyEmissionFactors();
}
