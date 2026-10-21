//------------------------------------------------------------------------------
/*! \file SIGSoilHeating.cpp
    \brief See SIGSoilHeating.h.
*/
#include "SIGSoilHeating.h"

SIGSoilHeating::SIGSoilHeating()
  : SoilHeating()
{
}

SIGSoilHeating::SIGSoilHeating(const SIGSoilHeating& rhs)
  : SoilHeating(rhs)
{
}

SIGSoilHeating& SIGSoilHeating::operator=(const SIGSoilHeating& rhs)
{
  SoilHeating::operator=(rhs);
  return *this;
}

void SIGSoilHeating::initializeMembers()
{
  SoilHeating::initializeMembers();
}

char* SIGSoilHeating::getMessage() const
{
  return SIGString::str2charptr(SoilHeating::getMessage());
}


void SIGSoilHeating::setSoilFamily(SoilFamily::SoilFamilyEnum soilFamily)
{
  getInputs().setSoilFamily(soilFamily);
}

void SIGSoilHeating::setSoilMoisture(double soilMoisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setSoilMoisture(soilMoisture, moistureUnits);
}

void SIGSoilHeating::setStartingSoilTemperature(double temperature, TemperatureUnits::TemperatureUnitsEnum temperatureUnits)
{
  getInputs().setStartingSoilTemperature(temperature, temperatureUnits);
}

void SIGSoilHeating::setDuffDepthPrefire(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits)
{
  getInputs().setDuffDepthPrefire(duffDepth, lengthUnits);
}

void SIGSoilHeating::setDuffDepthPostfire(double duffDepth, LengthUnits::LengthUnitsEnum lengthUnits)
{
  getInputs().setDuffDepthPostfire(duffDepth, lengthUnits);
}

void SIGSoilHeating::setDuffLoadPrefire(double duffLoad, LoadingUnits::LoadingUnitsEnum loadingUnits)
{
  getInputs().setDuffLoadPrefire(duffLoad, loadingUnits);
}

void SIGSoilHeating::setDuffMoisture(double duffMoisture, FractionUnits::FractionUnitsEnum moistureUnits)
{
  getInputs().setDuffMoisture(duffMoisture, moistureUnits);
}

void SIGSoilHeating::setDuffConsumed(double duffConsumed, FractionUnits::FractionUnitsEnum fractionUnits)
{
  getInputs().setDuffConsumed(duffConsumed, fractionUnits);
}

void SIGSoilHeating::setDuffHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits)
{
  getInputs().setDuffHeatEfficiency(efficiency, fractionUnits);
}

void SIGSoilHeating::setWoodLitterHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits)
{
  getInputs().setWoodLitterHeatEfficiency(efficiency, fractionUnits);
}

void SIGSoilHeating::setHerbShrubHeatEfficiency(double efficiency, FractionUnits::FractionUnitsEnum fractionUnits)
{
  getInputs().setHerbShrubHeatEfficiency(efficiency, fractionUnits);
}

void SIGSoilHeating::setFuelbedIgnited(bool isFuelbedIgnited)
{
  getInputs().setFuelbedIgnited(isFuelbedIgnited);
}

void SIGSoilHeating::clearFireIntensities()
{
  getInputs().clearFireIntensities();
}

void SIGSoilHeating::addFireIntensity(double woodLitterIntensity, double herbShrubIntensity, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits)
{
  getInputs().addFireIntensity(woodLitterIntensity, herbShrubIntensity, intensityUnits);
}

SoilFamily::SoilFamilyEnum SIGSoilHeating::getSoilFamily() const
{
  return getInputs().getSoilFamily();
}

double SIGSoilHeating::getSoilMoisture(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getSoilMoisture(moistureUnits);
}

double SIGSoilHeating::getStartingSoilTemperature(TemperatureUnits::TemperatureUnitsEnum temperatureUnits) const
{
  return getInputs().getStartingSoilTemperature(temperatureUnits);
}

double SIGSoilHeating::getDuffDepthPrefire(LengthUnits::LengthUnitsEnum lengthUnits) const
{
  return getInputs().getDuffDepthPrefire(lengthUnits);
}

double SIGSoilHeating::getDuffDepthPostfire(LengthUnits::LengthUnitsEnum lengthUnits) const
{
  return getInputs().getDuffDepthPostfire(lengthUnits);
}

double SIGSoilHeating::getDuffLoadPrefire(LoadingUnits::LoadingUnitsEnum loadingUnits) const
{
  return getInputs().getDuffLoadPrefire(loadingUnits);
}

double SIGSoilHeating::getDuffMoisture(FractionUnits::FractionUnitsEnum moistureUnits) const
{
  return getInputs().getDuffMoisture(moistureUnits);
}

double SIGSoilHeating::getDuffConsumed(FractionUnits::FractionUnitsEnum fractionUnits) const
{
  return getInputs().getDuffConsumed(fractionUnits);
}

double SIGSoilHeating::getDuffHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const
{
  return getInputs().getDuffHeatEfficiency(fractionUnits);
}

double SIGSoilHeating::getWoodLitterHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const
{
  return getInputs().getWoodLitterHeatEfficiency(fractionUnits);
}

double SIGSoilHeating::getHerbShrubHeatEfficiency(FractionUnits::FractionUnitsEnum fractionUnits) const
{
  return getInputs().getHerbShrubHeatEfficiency(fractionUnits);
}

bool SIGSoilHeating::getFuelbedIgnited() const
{
  return getInputs().getFuelbedIgnited();
}

int SIGSoilHeating::getNumberOfFireIntensities() const
{
  return getInputs().getNumberOfFireIntensities();
}

double SIGSoilHeating::getWoodLitterFireIntensity(int index, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits) const
{
  return getInputs().getWoodLitterFireIntensity(index, intensityUnits);
}

double SIGSoilHeating::getHerbShrubFireIntensity(int index, HeatSourceAndReactionIntensityUnits::HeatSourceAndReactionIntensityUnitsEnum intensityUnits) const
{
  return getInputs().getHerbShrubFireIntensity(index, intensityUnits);
}
