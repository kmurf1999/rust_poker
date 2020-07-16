# new make file
LIBTOOL=libtool
STATIC=-static
SRC        :=$(addprefix src/,deck.c hand_index.c)
OBJ        :=$(SRC:.c=.o)

LIB_FLAGS=-mmacosx-version-min=10.1
LIBNAME=libhandindexer.a

TEST_SRC=src/check-main.c
TESTCFLAGS=-mmacosx-version-min=10.7

$(LIBNAME): $(OBJ)
	$(LIBTOOL) $(STATIC) -o $@ $(OBJ)

%.o: %.c
	$(CC) $(LIB_FLAGS) -c -o $@ $<

test: $(LIBNAME) $(TEST_SRC)
	$(CC) $(TESTCFLAGS) -o $@ $(TEST_SRC) -L. -lhandindexer

clean:
	rm -f libhandindexer.a $(OBJ) test


