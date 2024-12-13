#include <iostream>
#include <sstream>
#include <functional>
#include <variant>
#include <fstream>
#include <type_traits>
#include <typeinfo>
#include <typeindex>
#include <nlohmann/json.hpp>
#include "behaveUnits.cpp"
#include "fuelModels.cpp"
#include "surface.h"
#include "surfaceFuelbedIntermediates.h"


class MyClass {
public:
  template<typename T>
  void printSum(T a, T b) {
    std::cout << "Sum: " << a + b << '\n';
  }
};

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
std::vector<float> convert<std::vector<float>>(const std::string& val) {
  std::vector<float> vec;

  std::istringstream iss(val);

  std::copy(std::istream_iterator<float>(iss),
	    std::istream_iterator<float>(),
	    std::back_inserter(vec));

  return vec;
}

template <>
std::string convert<std::string>(const std::string& val) {
  return val;
}

template<typename T, typename ... Args>
constexpr size_t num_args(void (T::*mem_fun)(Args...)) {
  return sizeof...(Args);
}

template<typename T, size_t N>
void copy_to_array(std::vector<T> & vector, std::array<T, N> & array) {
  std::copy_n(vector.begin(), N, array.begin());
}

template <typename RetType, typename T, typename... Args>
auto wrapFunction( RetType (T::*func)(Args...)) {

  // return [func](T* obj, std::tuple<Args...> args_tuple) {
  return [func](T* obj, std::tuple<Args...> args_tuple) -> RetType {

    //static_cast<MyClass>(obj);
    auto new_tuple = std::tuple_cat(std::tuple<T*>{obj}, args_tuple);
    return std::apply(func, new_tuple);
  };
}

template<std::size_t N, std::size_t remaining, typename T, typename ... Ts>
struct pack_tuple
{
  static std::tuple<T, Ts...> execute(std::array<std::string, N> v) {

    std::tuple head = { convert<T>(v.at(N - remaining)) };
    return std::tuple_cat(head, pack_tuple<N, remaining - 1, Ts...>::execute(v));

  };
};

template<std::size_t N, typename T>
struct pack_tuple<N, 1, T>
{
  static std::tuple<T> execute(std::array<std::string, N> v) {
    return { convert<T>(v.at(N - 1)) };
  };
};

template<size_t N, typename ... Ts>
void pack(std::tuple<Ts...> & out, std::array<std::string, N> v)
{
  auto result = pack_tuple<N, N, Ts...>::execute(v);
  out.swap(result);
}

template<typename RetType, typename T, typename ...Args>
auto func_args_tuple(RetType(T::*func)(Args...))
{
  return std::tuple<Args...>();
}

template<size_t N, typename RetType, typename T, typename ...Args>
auto packed_tuple(RetType(T::*func)(Args...), T* obj, std::vector<std::string> vec_args) {
  std::array<std::string, N> array_args;
  copy_to_array(vec_args, array_args);
  auto tuple_args = func_args_tuple(func);
  pack(tuple_args, array_args);
  return tuple_args;
}

int main() {
  MyClass c;
  auto func = wrapFunction(&MyClass::printSum<int>);
  const size_t N_args = num_args(&MyClass::printSum<int>);
  std::vector<std::string> vec_args {"5", "7"};
  auto tuple_args = packed_tuple<N_args>(&MyClass::printSum<int>, &c, vec_args);
  func(&c, tuple_args);

  return 0;
}
