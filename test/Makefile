# SPDX-License-Identifier: MPL-2.0

BINARY_CACHE_DIR := /opt/linux_binary_cache
VDSO_DIR := ../target
VDSO_LIB := $(VDSO_DIR)/vdso64.so
MKFILE_PATH := $(abspath $(lastword $(MAKEFILE_LIST)))
CUR_DIR := $(patsubst %/,%,$(dir $(MKFILE_PATH)))
ATOMIC_WGET := $(CUR_DIR)/../tools/atomic_wget.sh
BUILD_DIR := $(CUR_DIR)/build
INITRAMFS := $(BUILD_DIR)/initramfs
INITRAMFS_FILELIST := $(BUILD_DIR)/initramfs.filelist
INITRAMFS_SKIP_GZIP ?= 0
ifeq ($(INITRAMFS_SKIP_GZIP),1)
INITRAMFS_IMAGE := $(BUILD_DIR)/initramfs.cpio
else
INITRAMFS_IMAGE := $(BUILD_DIR)/initramfs.cpio.gz
endif
EXT2_IMAGE := $(BUILD_DIR)/ext2.img
EXFAT_IMAGE := $(BUILD_DIR)/exfat.img
METIS_IMAGE := $(BUILD_DIR)/metis.img
INITRAMFS_EMPTY_DIRS := \
	$(INITRAMFS)/root \
	$(INITRAMFS)/tmp \
	$(INITRAMFS)/opt \
	$(INITRAMFS)/proc \
	$(INITRAMFS)/dev \
	$(INITRAMFS)/ext2 \
	$(INITRAMFS)/exfat
INITRAMFS_ALL_DIRS := \
	$(INITRAMFS)/etc \
	$(INITRAMFS)/lib/x86_64-linux-gnu \
	$(INITRAMFS)/lib64 \
	$(INITRAMFS)/bin \
	$(INITRAMFS)/sbin \
	$(INITRAMFS)/usr/bin \
	$(INITRAMFS)/usr/local \
	$(INITRAMFS)/test \
	$(INITRAMFS)/benchmark \
	$(INITRAMFS_EMPTY_DIRS)
SYSCALL_TEST_DIR := $(INITRAMFS)/opt/syscall_test

.PHONY: all
all: build

$(INITRAMFS)/lib/x86_64-linux-gnu: | $(VDSO_LIB)
	@mkdir -p $@
	@cp -L /lib/x86_64-linux-gnu/libc.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libstdc++.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libm.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libgcc_s.so.1 $@
	@cp -L /lib/x86_64-linux-gnu/libpthread.so.0 $@
	@# required for benchmarks
	@cp -L /lib/x86_64-linux-gnu/libcrypto.so.3 $@
	@cp -L /lib/x86_64-linux-gnu/libcrypt.so.1 $@
	@cp -L /lib/x86_64-linux-gnu/libdl.so.2 $@
	@cp -L /lib/x86_64-linux-gnu/libz.so.1 $@
	@cp -L /lib/x86_64-linux-gnu/libmvec.so.1 $@
	@cp -L /usr/local/benchmark/iperf/lib/libiperf.so.0 $@
	@# required for LMbench-network
	@cp -L /lib/x86_64-linux-gnu/libtirpc.so.3 $@
	@cp -L /lib/x86_64-linux-gnu/libgssapi_krb5.so.2 $@
	@cp -L /lib/x86_64-linux-gnu/libkrb5.so.3 $@
	@cp -L /lib/x86_64-linux-gnu/libk5crypto.so.3 $@
	@cp -L /lib/x86_64-linux-gnu/libcom_err.so.2 $@
	@cp -L /lib/x86_64-linux-gnu/libkrb5support.so.0 $@
	@cp -L /lib/x86_64-linux-gnu/libkeyutils.so.1 $@
	@cp -L /lib/x86_64-linux-gnu/libresolv.so.2 $@
	@# required for LevelDB-db_bench_sqlite3
	@cp -L /lib/x86_64-linux-gnu/libsqlite3.so $@
	@# required for memcached
	@cp -L /lib/x86_64-linux-gnu/libevent-2.1.so.7 $@
	@# required for VDSO
	@cp -L $(VDSO_LIB) $@
	@cp -L /lib/x86_64-linux-gnu/libdb-5.3.so $@
	@cp -L /lib/x86_64-linux-gnu/libICE.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libSM.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libX11.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libXau.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libXdmcp.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libXext.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libXmu.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libXt.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libbsd.so.0 $@
	@cp -L /lib/x86_64-linux-gnu/libgomp.so.1 $@
	@cp -L /lib/x86_64-linux-gnu/libmd.so.0 $@
	@cp -L /lib/x86_64-linux-gnu/libxcb.so.1 $@
	@cp -L /lib64/ld-linux-x86-64.so.2 $@
	@cp -L /root/tz_utils/gcc-4.8.5/install/lib64/libstdc++.so.6 $@
	@cp -L /lib/x86_64-linux-gnu/libuuid.so.1 $@

$(VDSO_LIB): | $(VDSO_DIR) $(BINARY_CACHE_DIR)/vdso64.so
	@# TODO: use a custom compiled vdso.so file in the future.
	@cp $(BINARY_CACHE_DIR)/vdso64.so $@
	
$(VDSO_DIR):
	@mkdir -p $@
	
$(BINARY_CACHE_DIR)/vdso64.so:
	@mkdir -p $(BINARY_CACHE_DIR)
	@$(ATOMIC_WGET) $@ "https://raw.githubusercontent.com/asterinas/linux_vdso/2a6d2db/vdso64.so"

$(INITRAMFS)/lib64:
	@mkdir -p $@
	@cp -L /lib64/ld-linux-x86-64.so.2 $@

$(INITRAMFS)/etc:
	@mkdir -p $@
	@cp $(CUR_DIR)/etc/* $@

# Install busybox into /bin and /usr/bin.
$(INITRAMFS)/bin:
	@mkdir -p $@
	@/bin/busybox --install -s $@

$(INITRAMFS)/sbin:
	@mkdir -p $@
	@cp /sbin/ldconfig $@
	@cp /sbin/ldconfig.real $@

$(INITRAMFS)/usr/bin: | $(INITRAMFS)/bin
	@mkdir -p $@
	@cp /usr/bin/busybox $@

$(INITRAMFS)/usr/local:
	@mkdir -p $@
	@# cp -r /usr/local/nginx $@
	@# cp -r /usr/local/redis $@
	@# cp -r /usr/local/memcached $@

.PHONY: $(INITRAMFS)/test
$(INITRAMFS)/test:
	@$(MAKE) --no-print-directory -C apps

$(INITRAMFS)/benchmark: | $(INITRAMFS)/benchmark/bin
	@cp -rf $(CUR_DIR)/benchmark/* $@

$(INITRAMFS)/benchmark/bin:
	@mkdir -p $@
	@mkdir -p $@/../apache/logs
	@# Metis with RadixVM
	@cp -r /root/tz_utils/mm-scalability-benchmark/mosbench/metis_with_radixvm/metis/obj/app $@/metis
	@# cp -r /root/tz_utils/mm-scalability-benchmark/mosbench/metis/obj/app $@/metis
	@cp -r /root/tz_utils/mm-scalability-benchmark/mosbench/metis/run_in_vm.sh $@/metis
	@mkdir -p $@/metis/data
	@mkdir -p $@/psearchy
	@cp /root/tz_utils/mm-scalability-benchmark/mosbench/psearchy/mkdb/pedsort $@/psearchy
	@cp /root/tz_utils/mm-scalability-benchmark/mosbench/psearchy/create_dirs.sh $@/psearchy
	@cp /root/tz_utils/mm-scalability-benchmark/mosbench/psearchy/run_in_vm.sh $@/psearchy
	@bash /root/tz_utils/mm-scalability-benchmark/parsec-3.0/utils/init_run_dir.sh
	@cp /root/tz_utils/mm-scalability-benchmark/parsec-3.0/utils/run_in_vm.sh $@/
	@cp /root/tz_utils/vm_mount_disk.sh $@/../..
	
# Make necessary directories.
$(INITRAMFS_EMPTY_DIRS):
	@mkdir -p $@

.PHONY: $(SYSCALL_TEST_DIR)
$(SYSCALL_TEST_DIR):
	@$(MAKE) --no-print-directory -C syscall_test

.PHONY: $(INITRAMFS_IMAGE)
$(INITRAMFS_IMAGE): $(INITRAMFS_FILELIST)
	@if ! cmp -s $(INITRAMFS_FILELIST) $(INITRAMFS_FILELIST).previous || ! test -f $@; then \
		echo "Generating the initramfs image..."; \
		cp -f $(INITRAMFS_FILELIST) $(INITRAMFS_FILELIST).previous; \
		( \
			cd $(INITRAMFS); \
			# `$(INITRAMFS_FILELIST)` contains files' last modification \
			# time in the first column, and files' relative path to \
			# `$(INITRAMFS)` in the second column. This prunes the first \
			# column and passes the second column to `cpio`. \
			cut -d " " -f 2- $(INITRAMFS_FILELIST) | \
				cpio -o -H newc | \
					if [ "$(INITRAMFS_SKIP_GZIP)" != 1 ]; then \
						gzip; \
					else \
						cat; \
					fi \
		) > $@; \
	fi

.PHONY: $(INITRAMFS_FILELIST)
# If the BUILD_SYSCALL_TEST variable is set, we should depend on the
# sub make output to do incremental building.
ifeq ($(BUILD_SYSCALL_TEST), 1)
$(INITRAMFS_FILELIST): | $(INITRAMFS_ALL_DIRS) $(SYSCALL_TEST_DIR)
else
$(INITRAMFS_FILELIST): | $(INITRAMFS_ALL_DIRS)
endif
	@(cd $(INITRAMFS); find . -printf "%T@ %p\n") > $(INITRAMFS_FILELIST)

$(EXT2_IMAGE):
	@dd if=/dev/zero of=$(EXT2_IMAGE) bs=2G count=1
	@mke2fs $(EXT2_IMAGE)

$(EXFAT_IMAGE):
	@fallocate -l 64M $(EXFAT_IMAGE)
	@mkfs.exfat $(EXFAT_IMAGE)

$(METIS_IMAGE):
	@dd if=/dev/zero of=$(METIS_IMAGE) bs=2G count=3 iflag=fullblock
	@mkfs.ext2 $(METIS_IMAGE)
	@mkdir -p /mnt/metis_mount
	@mount -t ext2 $(METIS_IMAGE) /mnt/metis_mount
	@mkdir -p /mnt/metis_mount/wr
	@cp /root/tz_utils/mm-scalability-benchmark/mosbench/metis/data/wr/800MB.txt /mnt/metis_mount/wr
	@cp -r /root/tz_utils/linux_kernel/linux-2.6.35 /mnt/metis_mount
	@bash /root/tz_utils/mm-scalability-benchmark/parsec-3.0/utils/init_input_dir.sh
	@tree /mnt/metis_mount/parsec-data
	@du -d 1 -h /mnt/metis_mount/
	@umount /mnt/metis_mount
	@rm -rf /mnt/metis_mount/

.PHONY: build
build: $(INITRAMFS_IMAGE) $(EXT2_IMAGE) $(EXFAT_IMAGE) $(METIS_IMAGE)

.PHONY: format
format:
	@$(MAKE) --no-print-directory -C apps format

.PHONY: check
check:
	@$(MAKE) --no-print-directory -C apps check

.PHONY: clean
clean:
	@rm -rf $(BUILD_DIR)