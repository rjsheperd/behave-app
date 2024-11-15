#ifndef TESTUTILS_H
#define TESTUTILS_H

#include <map>
#include <string>
#include <vector>

struct TestInfo
{
  int numTotalTests = 0;
  int numFailed = 0;
  int numPassed = 0;
};

class CSVData {
 public:

  static CSVData* factory(std::string const& filename);

  std::vector<std::string> csvHeaders;
  std::vector<std::map<std::string, std::string>> csvStringRows;
  std::vector<std::map<std::string, double>> csvDoubleRows;
};

int parseCSVFile(std::string const & filename, CSVData & csvData);

int parseCSVFile(std::string filename,
                 std::vector<std::string> & csvHeaders,
                 std::vector<std::map<std::string, std::string>> & csvStringRows,
                 std::vector<std::map<std::string, double>> & csvDoubleRows);

void printCSVData(std::vector<std::string> csvHeaders,
                  std::vector<std::map<std::string, std::string>> csvStringRows,
                  std::vector<std::map<std::string, double>> csvDoubleRows);

bool beginsWithNumber(std::string const & str);

bool areClose(const double observed, const double expected, const double epsilon);

double roundToSixDecimalPlaces(const double numberToBeRounded);

void reportTestResult(int row, struct TestInfo& testInfo, const std::string testName, const double observed, const double expected, const double epsilon);


// void reportTestResult(const std::string testName, const int observed, const int expected);

// void reportTestResult(const std::string testName, const double observed, const double expected, const double epsilon);


#endif
