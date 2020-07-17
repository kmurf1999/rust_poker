project(handindexer)
cmake_minimum_required(VERSION 3.8)
set(ignoreMe "${CMAKE_ASM_COMPILER}${CMAKE_ASM_FLAGS}")

set(SOURCES
  ${CMAKE_SOURCE_DIR}/src/hand_index.c
  ${CMAKE_SOURCE_DIR}/src/deck.c
)

include_directories(${CMAKE_SOURCE_DIR}/src)
add_library(handindexer ${SOURCES})
install(TARGETS handindexer DESTINATION ".")
