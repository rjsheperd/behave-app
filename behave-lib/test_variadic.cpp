#include <iostream>
#include <functional> 
#include <variant> 
#include <fstream> 
#include <type_traits> 
#include <typeinfo> 
#include <typeindex> 

template <typename T>
T convert(const std::string& val);

template <>
int convert<int>(const std::string& val) {
  return std::stoi(val);
}

template <>
double convert<double>(const std::string& val) {
  return std::stod(val);
}

template <>
bool convert<bool>(const std::string& val) {
  return val == "true" || val == "1";
}

template <>
std::string convert<std::string>(const std::string& val) {
  return val;
}

//// Original
//template<>
//std::tuple<> toTupleImpl(int index, std::vector<std::string> args) {
//  return {};
//}
//
//template<typename T, typename... Args>
//std::tuple<T, Args...> toTupleImpl(int index, std::vector<std::string> args) {
//  if (index < args.size()) {
//    return std::tuple_cat(std::make_tuple(convert<T>(args.at(index))), toTupleImpl<Args...>(index + 1, args));
//  } else {
//    return {};
//  }
//} 

// GPT
// base case
// template<>
// std::tuple<> toTupleImpl<>(int, std::vector<std::string>&) {
//   return std::tuple<>();
// }

template<typename T>
std::tuple<T> toTupleImpl(int index, const std::vector<std::string>& args) {
  return { convert<T>(args.at(index)) };
}

// recursive case
template<typename T, typename... Args>
std::tuple<T, Args...> toTupleImpl(int index, std::vector<std::string>& args) {
  if ((index > 0) && ((index + 1) == args.size())) {
    return { convert<T>(args.at(index)) };
  } else if (index < args.size()) {
    return std::tuple_cat(std::make_tuple(convert<T>(args.at(index))), toTupleImpl<Args...>(index + 1, args));
  } else {
    return std::tuple<Args...>();
  }
}

// recursive case
template<typename T, typename... Args>
std::tuple<T, Args...> toTupleImpl(int index, std::vector<std::string>& args) {
  if ((index > 0) && ((index + 1) == args.size())) {
    return { convert<T>(args.at(index)) };
  } else if (index < args.size()) {
    return std::tuple_cat(std::make_tuple(convert<T>(args.at(index))), toTupleImpl<Args...>(index + 1, args));
  } else {
    return std::tuple<Args...>();
  }
}

// recursive case
template<typename... Args>
std::tuple<Args...> toTuple(std::vector<std::string> args) {
  std::tuple<Args...> tuple_args;
  toTupleImpl<Args...>(&tuple_args, 0, args);
}

int main()
{
  toTuple<std::string, int, double>(std::vector<std::string>({"test", "1", "2.0"}));

  return 0;
}
