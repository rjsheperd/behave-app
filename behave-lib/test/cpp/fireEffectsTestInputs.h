// Shared by testFuelConsumption and testSoilHeating: reads one row of
// test/csv/fuel_consumption.csv or soil_heating.csv into the SIG adapter, and
// the FOFEM outputs each row expects. The CSVs come from FOFEM 6.9.4's golden
// master (fofem repository test/golden, through behave-mirror's testFofem
// batch reader); the enum name tables are generated from
// fireEffectsInputEnums.h by hatchet.
#ifndef FIREEFFECTSTESTINPUTS_H
#define FIREEFFECTSTESTINPUTS_H

#include <cmath>
#include <map>
#include <string>
#include <vector>

#include "SIGFuelConsumption.h"
#include "testUtils.h"

static const std::map<std::string, FuelConsumptionRegion::FuelConsumptionRegionEnum> fuelConsumptionRegionByName{
  {"InteriorWest", FuelConsumptionRegion::InteriorWest},
  {"PacificWest", FuelConsumptionRegion::PacificWest},
  {"NorthEast", FuelConsumptionRegion::NorthEast},
  {"SouthEast", FuelConsumptionRegion::SouthEast}};

static const std::map<std::string, CoverGroup::CoverGroupEnum> coverGroupByName{
  {"None", CoverGroup::None},
  {"Grass", CoverGroup::Grass},
  {"Shrub", CoverGroup::Shrub},
  {"ShrubChaparral", CoverGroup::ShrubChaparral},
  {"Sagebrush", CoverGroup::Sagebrush},
  {"PonderosaPine", CoverGroup::PonderosaPine},
  {"Pocosin", CoverGroup::Pocosin},
  {"BalsamBlackRedWhiteSpruce", CoverGroup::BalsamBlackRedWhiteSpruce},
  {"RedJackPine", CoverGroup::RedJackPine},
  {"WhitePineHemlock", CoverGroup::WhitePineHemlock},
  {"PineFlatwoods", CoverGroup::PineFlatwoods},
  {"CoastalPlain", CoverGroup::CoastalPlain}};

static const std::map<std::string, BurnSeason::BurnSeasonEnum> burnSeasonByName{
  {"Spring", BurnSeason::Spring},
  {"Summer", BurnSeason::Summer},
  {"Fall", BurnSeason::Fall},
  {"Winter", BurnSeason::Winter}};

static const std::map<std::string, FuelCategory::FuelCategoryEnum> fuelCategoryByName{
  {"Natural", FuelCategory::Natural},
  {"Piles", FuelCategory::Piles},
  {"Slash", FuelCategory::Slash}};

static const std::map<std::string, DuffMoistureMethod::DuffMoistureMethodEnum> duffMoistureMethodByName{
  {"EntireDuffMoisture", DuffMoistureMethod::EntireDuffMoisture},
  {"LowerDuffMoisture", DuffMoistureMethod::LowerDuffMoisture},
  {"NationalFireDangerRatingThousandHour", DuffMoistureMethod::NationalFireDangerRatingThousandHour},
  {"AdjustedNationalFireDangerRatingThousandHour", DuffMoistureMethod::AdjustedNationalFireDangerRatingThousandHour}};

static const std::map<std::string, DuffType::DuffTypeEnum> duffTypeByName{
  {"PeatLow", DuffType::PeatLow},
  {"PeatHigh", DuffType::PeatHigh},
  {"DuffLow", DuffType::DuffLow},
  {"DuffHigh", DuffType::DuffHigh}};

static const std::map<std::string, SoilFamily::SoilFamilyEnum> soilFamilyByName{
  {"LoamySkeletal", SoilFamily::LoamySkeletal},
  {"FineSilt", SoilFamily::FineSilt},
  {"Fine", SoilFamily::Fine},
  {"CoarseSilt", SoilFamily::CoarseSilt},
  {"CoarseLoamy", SoilFamily::CoarseLoamy},
  {"QuartzSand", SoilFamily::QuartzSand},
  {"BasaltSand", SoilFamily::BasaltSand}};

inline void setFuelConsumptionInputs(SIGFuelConsumption& run, std::map<std::string, double>& d, std::map<std::string, std::string>& s)
{
  const LoadingUnits::LoadingUnitsEnum loadUnits = s["fuelLoadUnits"] == "TonsPerAcre" ? LoadingUnits::TonsPerAcre : LoadingUnits::KilogramsPerSquareMeter;
  const FractionUnits::FractionUnitsEnum percent = FractionUnits::Percent;
  run.setRegion(fuelConsumptionRegionByName.at(s["region"]));
  run.setCoverGroup(coverGroupByName.at(s["coverGroup"]));
  run.setBurnSeason(burnSeasonByName.at(s["burnSeason"]));
  run.setFuelCategory(fuelCategoryByName.at(s["fuelCategory"]));
  run.setDuffMoistureMethod(duffMoistureMethodByName.at(s["duffMoistureMethod"]));
  run.setDuffType(duffTypeByName.at(s["duffType"]));
  run.setEmissionFactorGroups((int)d["flamingEmissionFactorGroup"], (int)d["coarseWoodyEmissionFactorGroup"], (int)d["duffEmissionFactorGroup"]);
  run.setFuelLoadLitter(d["fuelLoadLitter"], loadUnits);
  run.setFuelLoadOneHour(d["fuelLoadOneHour"], loadUnits);
  run.setFuelLoadTenHour(d["fuelLoadTenHour"], loadUnits);
  run.setFuelLoadHundredHour(d["fuelLoadHundredHour"], loadUnits);
  run.setFuelLoadSoundThreeToSixInch(d["fuelLoadSoundThreeToSixInch"], loadUnits);
  run.setFuelLoadSoundSixToNineInch(d["fuelLoadSoundSixToNineInch"], loadUnits);
  run.setFuelLoadSoundNineToTwentyInch(d["fuelLoadSoundNineToTwentyInch"], loadUnits);
  run.setFuelLoadSoundTwentyInchAndLarger(d["fuelLoadSoundTwentyInchAndLarger"], loadUnits);
  run.setFuelLoadRottenThreeToSixInch(d["fuelLoadRottenThreeToSixInch"], loadUnits);
  run.setFuelLoadRottenSixToNineInch(d["fuelLoadRottenSixToNineInch"], loadUnits);
  run.setFuelLoadRottenNineToTwentyInch(d["fuelLoadRottenNineToTwentyInch"], loadUnits);
  run.setFuelLoadRottenTwentyInchAndLarger(d["fuelLoadRottenTwentyInchAndLarger"], loadUnits);
  run.setFuelLoadDuff(d["fuelLoadDuff"], loadUnits);
  run.setFuelLoadHerbaceous(d["fuelLoadHerbaceous"], loadUnits);
  run.setFuelLoadShrub(d["fuelLoadShrub"], loadUnits);
  run.setFuelLoadCrownFoliage(d["fuelLoadCrownFoliage"], loadUnits);
  run.setFuelLoadCrownBranch(d["fuelLoadCrownBranch"], loadUnits);
  run.setDuffDepth(d["duffDepth"], s["duffDepthUnits"] == "Inches" ? LengthUnits::Inches : LengthUnits::Centimeters);
  run.setPercentOfCrownBurned(d["percentOfCrownBurned"], percent);
  run.setMoistureDuff(d["moistureDuff"], percent);
  run.setMoistureTenHour(d["moistureTenHour"], percent);
  run.setMoistureThousandHour(d["moistureThousandHour"], percent);
  run.setMoistureLitter(d["moistureLitter"], percent);
}

// FOFEM's golden column -> what the adapter reports for it (t/ac, lb/ac,
// percent, inches, seconds, as runfofem's report)
inline std::vector<std::pair<std::string, double>> fuelConsumptionObservations(const SIGFuelConsumption& run)
{
  typedef FuelConsumptionComponent C;
  const LoadingUnits::LoadingUnitsEnum tpa = LoadingUnits::TonsPerAcre;
  const LoadingUnits::LoadingUnitsEnum lba = LoadingUnits::PoundsPerAcre;
  const char* prefixes[] = { "Lit", "DW1", "DW10", "DW100", "Snd3", "Snd6", "Snd9", "Snd20", "Rot3", "Rot6", "Rot9", "Rot20",
    "Duf", "Her", "Shr", "Fol", "Bra" };
  const C::FuelConsumptionComponentEnum components[] = { C::Litter, C::OneHour, C::TenHour, C::HundredHour,
    C::SoundThreeToSixInch, C::SoundSixToNineInch, C::SoundNineToTwentyInch, C::SoundTwentyInchAndLarger,
    C::RottenThreeToSixInch, C::RottenSixToNineInch, C::RottenNineToTwentyInch, C::RottenTwentyInchAndLarger, C::Duff,
    C::Herbaceous, C::Shrub, C::CrownFoliage, C::CrownBranch };
  const char* emissions[] = { "PM10", "PM25", "CH4", "CO", "CO2", "NOX", "SO2" };
  const EmissionSpecies::EmissionSpeciesEnum species[] = { EmissionSpecies::ParticulateMatterTen,
    EmissionSpecies::ParticulateMatterTwoPointFive, EmissionSpecies::Methane, EmissionSpecies::CarbonMonoxide,
    EmissionSpecies::CarbonDioxide, EmissionSpecies::NitrogenOxides, EmissionSpecies::SulfurDioxide };
  std::vector<std::pair<std::string, double>> observed;
  for (int c = 0; c < 17; c++)
  {
    observed.push_back({ std::string(prefixes[c]) + "Con", run.getFuelLoadConsumed(components[c], tpa) });
    observed.push_back({ std::string(prefixes[c]) + "Pos", run.getFuelLoadPostfire(components[c], tpa) });
  }
  observed.push_back({ "MSE", run.getMineralSoilExposed(FractionUnits::Percent) });
  observed.push_back({ "DufDepCon", run.getDuffDepthReduction(LengthUnits::Inches) });
  observed.push_back({ "DufDepPos", run.getDuffDepthPostfire(LengthUnits::Inches) });
  for (int e = 0; e < 7; e++)
  {
    observed.push_back({ std::string(emissions[e]) + "F", run.getFlamingEmission(species[e], lba) });
    observed.push_back({ std::string(emissions[e]) + "S", run.getSmolderingEmission(species[e], lba) });
  }
  observed.push_back({ "FlaDur", run.getFlamingDuration(TimeUnits::Seconds) });
  observed.push_back({ "SmoDur", run.getSmolderingDuration(TimeUnits::Seconds) });
  observed.push_back({ "FlaCon", run.getFuelLoadConsumedFlaming(tpa) });
  observed.push_back({ "SmoCon", run.getFuelLoadConsumedSmoldering(tpa) });
  return observed;
}

// FOFEM's golden values are floats written with 9 digits: compare to 1e-4,
// relative, as behave-mirror's testFofem does
inline double fofemTolerance(double expected)
{
  return 1.0e-4 * std::fabs(expected) + 1.0e-5;
}

#endif
