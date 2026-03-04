// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Copyright 2026 Oxide Computer Company

/// Decoded POST code from an AMD Turin processor.
pub enum PostCode {
    /// PMU (Platform Management Unit) training progress code.
    Pmu(PmuCode),
    /// ASP bootloader status code.
    Bootloader(BootloaderInfo),
    /// ABL (AGESA Bootloader) test point with a known meaning.
    Abl(AblCode),
    /// Code in the ABL range (0xEA00xxxx) but not in the lookup table.
    UnknownAbl(u32),
    /// Completely unrecognised POST code.
    Unknown(u32),
}

/// PMU training progress extracted from a 0xEA01xxxx code.
pub struct PmuCode {
    /// The raw 32-bit POST code.
    pub code: u32,
    /// UMC channel number (0–0xB).
    pub umc_channel: u16,
    /// Physical board DIMM slot letter (A–L), or "unknown".
    pub board_dimm: &'static str,
    /// Training phase (0 or 1).
    pub training_phase: u16,
    /// PMU progress code (low byte).
    pub progress_code: u16,
}

/// ASP bootloader status information.
pub struct BootloaderInfo {
    /// The raw 32-bit POST code.
    pub code: u32,
    /// Which ASP component: "ASP FMC", "ASP BL2", or "ASP TEE".
    pub source: &'static str,
    /// Symbolic name of the status (e.g. "BL_OK").
    pub name: &'static str,
    /// Numeric status byte.
    pub status: u8,
    /// Human-readable description.
    pub description: &'static str,
}

/// A known ABL test point.
pub struct AblCode {
    /// The raw 32-bit POST code.
    pub code: u32,
    /// Symbolic name (e.g. "TpProcMemDramInit").
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
}

impl PostCode {
    /// Format the decoded POST code as lines suitable for printing.
    pub fn lines(&self) -> Vec<String> {
        match self {
            PostCode::Pmu(pmu) => {
                let phase = match pmu.training_phase {
                    0 => "phase 0",
                    1 => "phase 1",
                    _ => "unknown",
                };
                vec![
                    format!("PMU Code: 0x{:08x}", pmu.code),
                    format!(
                        "  UMC channel:    0x{:x} ({})",
                        pmu.umc_channel, pmu.umc_channel
                    ),
                    format!("  Board DIMM:     {}", pmu.board_dimm),
                    format!(
                        "  Training phase: {} ({phase})",
                        pmu.training_phase
                    ),
                    format!(
                        "  Progress code:  0x{:02x} ({})",
                        pmu.progress_code, pmu.progress_code
                    ),
                ]
            }
            PostCode::Bootloader(info) => vec![
                format!("Bootloader Code: 0x{:08x}", info.code),
                format!("  Source:  {}", info.source),
                format!("  Status:  {} (0x{:02x})", info.name, info.status),
                format!("  Detail:  {}", info.description),
            ],
            PostCode::Abl(abl) => vec![
                format!("ABL Code: 0x{:08x}", abl.code),
                format!("  Name:   {}", abl.name),
                format!("  Detail: {}", abl.description),
            ],
            PostCode::UnknownAbl(code) => vec![
                format!("ABL Code: 0x{code:08x}"),
                "  Unknown ABL test point (no matching entry)".to_string(),
            ],
            PostCode::Unknown(code) => {
                vec![format!("Unknown POST code: 0x{code:08x}")]
            }
        }
    }
}

/// Decode a 32-bit AMD Turin POST code.
pub fn decode(code: u32) -> PostCode {
    // PMU codes: 0xEA01xyzz
    if code & 0xffff0000 == 0xea010000 {
        let lower = (code & 0xffff) as u16;
        let x = (lower >> 12) & 0xf;
        let y = (lower >> 8) & 0xf;
        let zz = lower & 0xff;
        return PostCode::Pmu(PmuCode {
            code,
            umc_channel: x,
            board_dimm: umc_to_board_dimm(x),
            training_phase: y,
            progress_code: zz,
        });
    }

    // Bootloader codes
    if let Some(info) = decode_bootloader(code) {
        return PostCode::Bootloader(info);
    }

    // ABL test point lookup
    if let Some((name, description)) = look_up_abl(code) {
        return PostCode::Abl(AblCode { code, name, description });
    }

    // Unknown but in the ABL range
    if code & 0xff000000 == 0xea000000 {
        return PostCode::UnknownAbl(code);
    }

    PostCode::Unknown(code)
}

fn umc_to_board_dimm(umc: u16) -> &'static str {
    match umc {
        0 => "C",
        1 => "E",
        2 => "F",
        3 => "A",
        4 => "B",
        5 => "D",
        6 => "I",
        7 => "K",
        8 => "L",
        9 => "G",
        10 => "H",
        11 => "J",
        _ => "unknown",
    }
}

fn decode_bootloader(code: u32) -> Option<BootloaderInfo> {
    let source = match code & 0xff000000 {
        0xaa000000 => "ASP FMC",
        0xee000000 => {
            if code & 0xfff00000 == 0xee100000 {
                "ASP BL2"
            } else {
                "ASP TEE"
            }
        }
        0xed000000 => "ASP TEE",
        _ => return None,
    };

    let status = (code & 0xff) as u8;
    let (name, description) = look_up_bootloader_status(status);

    Some(BootloaderInfo { code, source, name, status, description })
}

fn look_up_bootloader_status(status: u8) -> (&'static str, &'static str) {
    match status {
        // Error codes 0x00-0x9F
        0x00 => ("BL_OK", "Success"),
        0x01 => ("BL_ERR_GENERIC", "Generic Error Code"),
        0x02 => ("BL_ERR_MEMORY", "Generic Memory Error"),
        0x03 => ("BL_ERR_BUFFER_OVERFLOW", "Buffer Overflow"),
        0x04 => ("BL_ERR_INVALID_PARAMETER", "Invalid Parameter(s)"),
        0x05 => ("BL_ERR_DATA_LENGTH", "Invalid Data Length"),
        0x06 => ("BL_ERR_DATA_ALIGNMENT", "Data Alignment Error"),
        0x07 => ("BL_ERR_NULL_PTR", "Null Pointer Error"),
        0x08 => ("BL_ERR_FUNCTION_NOT_SUPPORTED", "Unsupported Function"),
        0x09 => ("BL_ERR_INVALID_SERVICE_ID", "Invalid Service ID"),
        0x0A => ("BL_ERR_INVALID_ADDRESS", "Invalid Address"),
        0x0B => ("BL_ERR_OUT_OF_RESOURCES", "Out of Resource Error"),
        0x0C => ("BL_ERR_TIMEOUT", "Timeout"),
        0x0D => ("BL_ERR_DATA_ABORT", "Data abort exception"),
        0x0E => ("BL_ERR_PREFETCH_ABORT", "Prefetch abort exception"),
        0x0F => ("BL_ERR_BOUNDARY_CHECK", "Out of Boundary Condition Reached"),
        0x10 => ("BL_ERR_DATA_CORRUPTION", "Data corruption"),
        0x11 => ("BL_ERR_INVALID_COMMAND", "Invalid command"),
        0x12 => {
            ("BL_ERR_INCORRECT_PACKAGE_TYPE", "Incorrect package type from BR")
        }
        0x13 => (
            "BL_ERR_GET_FW_HEADER",
            "Failed to retrieve FW header during validation",
        ),
        0x14 => ("BL_ERR_KEY_SIZE", "Key size not supported"),
        0x15 => ("BL_ERR_AGESA0", "Agesa0 verification error"),
        0x16 => ("BL_ERR_SMUFW", "SMU FW verification error"),
        0x17 => ("BL_ERR_OEMSIGNING", "OEM SIGNING KEY verification error"),
        0x18 => ("BL_ERR_FWVALIDATION", "Generic FW Validation error"),
        0x19 => ("BL_ERR_CCP_RSA", "RSA operation fail - bootloader"),
        0x1A => ("BL_ERR_CCP_PASSTHR", "CCP Passthrough operation failed"),
        0x1B => ("BL_ERR_CCP_AES", "AES operation fail"),
        0x1C => ("BL_ERR_STATE_SAVE", "CCP state save failed"),
        0x1D => ("BL_ERR_STATE_RESTORE", "CCP state restore failed"),
        0x1E => ("BL_ERR_SHA", "SHA256/384 operation fail"),
        0x1F => ("BL_ERR_ZLIB", "ZLib Decompression operation fail"),
        0x20 => ("BL_ERR_HMAC_SHA", "HMAC-SHA256/384 operation fail"),
        0x21 => {
            ("BL_ERR_INVALID_BOOT_SOURCE", "Boot source not recognized by PSP")
        }
        0x22 => ("BL_ERR_DIR_ENTRY_NOT_FOUND", "PSP directory entry not found"),
        0x23 => {
            ("BL_ERR_SPIROM_WRITE_FAIL", "PSP failed to set write enable latch")
        }
        0x24 => {
            ("BL_ERR_SPIROM_BUSY_TIMEOUT", "PSP timed out waiting for spirom")
        }
        0x25 => ("BL_ERR_CANNOT_FIND_BIOS_DIR", "Cannot find BIOS directory"),
        0x26 => ("BL_ERR_SPIROM_SIZE", "SpiRom is not valid"),
        0x27 => (
            "BL_ERR_SECURITY_STATE_DIFF",
            "Slave die has different security state from master",
        ),
        0x28 => ("BL_ERR_SMI_INIT_ERROR", "SMI interface init failure"),
        0x29 => ("BL_ERR_SMI_GENERIC", "SMI interface generic error"),
        0x2A => (
            "BL_ERR_INVALID_DIE_ID",
            "Invalid die ID executes MCM related function",
        ),
        0x2B => (
            "BL_ERR_INVALID_MCM_CONFIG",
            "Invalid MCM configuration table from bootrom",
        ),
        0x2C => ("BL_ERR_DETECT_BOOT_MODE", "Valid boot mode wasn't detected"),
        0x2D => ("BL_ERR_NVSTORAGE_INIT_FAILURE", "NVStorage init failure"),
        0x2E => ("BL_ERR_NVSTORAGE_GENERIC", "NVStorage generic error"),
        0x2F => ("BL_ERR_MCM_MORE_DATA", "MCM: slave has more data to send"),
        0x30 => ("BL_ERR_MCM_DATA_LENGTH", "MCM: data size exceeds 32B"),
        0x31 => ("BL_ERR_MCM_INVALID_ID", "Invalid client id for SVC MCM call"),
        0x32 => (
            "BL_ERR_MCM_INVALID_STATE",
            "MCM slave status register contains bad bits",
        ),
        0x33 => ("BL_ERR_MCM_NO_SLAVES", "MCM call in single die environment"),
        0x34 => {
            ("BL_ERR_PSP_SECURE_MAP", "PSP secure mapped to invalid segment")
        }
        0x35 => (
            "BL_ERR_NO_PHY_CORES_PRESENT",
            "No physical x86 cores found on die",
        ),
        0x36 => {
            ("BL_ERR_SECURE_OS_INSUF_SRAM", "Insufficient space for secure OS")
        }
        0x37 => (
            "BL_ERR_UNSUP_SYSHUB_TARGET_TYPE",
            "SYSHUB mapping memory target type not supported",
        ),
        0x38 => (
            "BL_ERR_UNMAP_PSP_SECURE_REGION",
            "Attempt to unmap permanent TLB to PSP secure region",
        ),
        0x39 => {
            ("BL_ERR_SMNMAP_FAILED", "Unable to map SMN address to AXI space")
        }
        0x3A => (
            "BL_ERR_SYSHUBMAP_FAILED",
            "Unable to map SYSHUB address to AXI space",
        ),
        0x3B => (
            "BL_ERR_CORECONFIG_COUNT_MISMATCH",
            "CCX/core count from bootrom inconsistent",
        ),
        0x3C => (
            "BL_ERR_UNCOMP_IMAGE_SIZE_MISMATCH",
            "Uncompressed image size mismatch",
        ),
        0x3D => (
            "BL_ERR_UNSUPPORTED_COMP_OPTION",
            "Compressed option used where not supported",
        ),
        0x3E => ("BL_ERR_FUSE_INFO", "Fuse info on all dies don't match"),
        0x3F => {
            ("BL_ERR_PSP_SMU_MSG_FAIL", "PSP->SMU message; SMU reported error")
        }
        0x40 => (
            "BL_ERR_POST_X86_RELEASE_TEST_FAIL",
            "RunPostX86ReleaseUnitTests failed",
        ),
        0x41 => {
            ("BL_ERR_PSP_SMU_INTERFACE", "PSP to SMU interface not available")
        }
        0x42 => {
            ("BL_ERR_TIMER_PARAM_OVERFLOW", "Timer wait parameter too large")
        }
        0x43 => {
            ("BL_ERR_TEST_HARNESS_MODULE", "Test harness module reported error")
        }
        0x44 => (
            "BL_ERR_VMG_INVALID_COMMAND",
            "x86 C2PMSG_0 interrupt with invalid command",
        ),
        0x45 => (
            "BL_ERR_BIOS_DIR_COOKIE_MISMATCH",
            "Failed to read BIOS Directory from SPI",
        ),
        0x46 => ("BL_ERR_SPL_LOOKUP", "Failed to find FW entry in SPL Table"),
        0x47 => (
            "BL_ERR_COMBO_BIOS_HEADER_MISMATCH",
            "Failed to read combo BIOS header",
        ),
        0x48 => ("BL_ERR_INVALID_SPL_VERSION", "SPL version mismatch"),
        0x49 => (
            "BL_ERR_LOAD_VALIDATE_APOB",
            "Error in Validate/Load AGESA APOB SVC call",
        ),
        0x4A => (
            "BL_ERR_WAIT_FOR_RESET_ASSERTION",
            "Correct fuse bits for DIAG_BL not set",
        ),
        0x4B => (
            "BL_ERR_UMC_NOT_INIT_BY_AGESA",
            "UmcProgramKeys() not called by AGESA",
        ),
        0x4C => (
            "BL_ERR_WHITE_LISTING",
            "Unconditional Unlock serial number failure",
        ),
        0x4D => (
            "BL_ERR_SYSHUBIF_REG_MISMATCH",
            "Syshub register readback mismatch",
        ),
        0x4E => (
            "BL_ERR_SECURE_FUSE_FAMILY_ID",
            "Family ID in MP0_SFUSE_SEC[7:3] not correct",
        ),
        0x4F => (
            "BL_ERR_NOT_GLOBAL_MASTER",
            "Operation can only be performed by GM",
        ),
        0x50 => (
            "BL_ERR_SMB_TIMEOUT_ACQR_HOST_SEMA",
            "Failed to acquire SMB host controller semaphore",
        ),
        0x51 => (
            "BL_ERR_SMB_TIMEOUT_WAIT_HOST_IDLE",
            "Timed out waiting for host idle",
        ),
        0x52 => (
            "BL_ERR_SMB_TIMEOUT_WAIT_SLAVE_IDLE",
            "Timed out waiting for slave idle",
        ),
        0x53 => {
            ("BL_ERR_SMB_HOST_BUSY", "Unable to kill current host transaction")
        }
        0x54 => (
            "BL_ERR_SMB_DEVICE_ERROR",
            "Illegal command, Unclaimed cycle, or Host timeout",
        ),
        0x55 => {
            ("BL_ERR_SMB_BUS_COLLISION", "SMBus transaction collision detected")
        }
        0x56 => (
            "BL_ERR_SMB_TRANSACTION_FAILED",
            "Transaction failed to start or complete",
        ),
        0x57 => (
            "BL_ERR_SMB_UNSOLICITED_INTR_RX",
            "Unsolicited SMBus interrupt received",
        ),
        0x58 => {
            ("BL_ERR_PSP_SMU_UNSUPPORTED_MSG", "Unsupported PSP-SMU message")
        }
        0x59 => {
            ("BL_ERR_PSP_SMU_CORRUPTED_TXFR", "Data corruption on SMU response")
        }
        0x5A => (
            "BL_ERR_MCM_STEADY_UNIT_TEST_FAILED",
            "MCM steady-state unit test failed",
        ),
        0x5B => ("BL_ERR_S3_ENTER_FAILED", "S3 Enter failed"),
        0x5C => (
            "BL_ERR_PSP_SMU_RESERVED_NOT_SET",
            "AGESA BL did not set PSP SMU reserved addresses",
        ),
        0x5D => (
            "BL_ERR_PSP_SMU_RESERVED_INVALID",
            "Reserved PSP/SMU memory region invalid",
        ),
        0x5E => ("BL_ERR_UNFUSED_PART", "CcxSecBisiEn not set in fuse RAM"),
        0x5F => ("BL_ERR_UNIT_TEST_UNEXPECTED_RESULT", "Unexpected result"),
        0x60 => ("BL_ERR_VMG_STORAGE_INIT_FAILED", "VMG Storage Init failed"),
        0x61 => ("BL_ERR_MBEDTLS_USER_APP", "Failure in mbedTLS user app"),
        0x62 => (
            "BL_ERR_FUSE_SMN_MAPPING_FAILED",
            "Error SMN mapping fuse register",
        ),
        0x63 => (
            "BL_ERR_FUSE_BURN_FAILED_SOC",
            "Fuse burn failed (internal SOC error)",
        ),
        0x64 => ("BL_ERR_FUSE_SENSE_TIMEOUT", "Fuse sense operation timed out"),
        0x65 => ("BL_ERR_FUSE_BURN_FAILED_TIMEOUT", "Fuse burn timed out"),
        0x66 => (
            "BL_ERR_PMU_FW_KEY",
            "PMU FW public key certificate loading/auth fails",
        ),
        0x67 => ("BL_ERR_FUSE_FW_ID_REVOKED", "PSP FW was revoked"),
        0x68 => {
            ("BL_ERR_PLATFORM_ID", "Platform model/vendor id fuse mismatch")
        }
        0x69 => ("BL_ERR_BIOS_KEY_REV_ID", "BIOS OEM public key revoked"),
        0x6A => (
            "BL_ERR_PSP_LV2_HEADER_NOT_MATCH",
            "PSP level 2 directory mismatch",
        ),
        0x6B => (
            "BL_ERR_BIOS_LV2_HEADER_NOT_MATCH",
            "BIOS level 2 directory mismatch",
        ),
        0x6C => ("BL_ERR_RESET_IMAGE_NOT_FOUND", "Reset image not found"),
        0x6D => ("BL_ERR_CCP_INIT", "CCP HAL initialization failed"),
        0x6E => ("BL_ERR_NVRAM_DRAM_FAIL", "Failure to copy NVRAM to DRAM"),
        0x6F => ("BL_ERR_INVALID_KEY_USAGE_FLAG", "Invalid key usage flag"),
        0x70 => ("BL_ERR_UNEXPECTED_FUSE_SET", "Unexpected fuse set"),
        0x71 => (
            "BL_ERR_RSMU_SECURITY_VIOLATION",
            "RSMU signaled security violation",
        ),
        0x72 => (
            "BL_ERR_WAFL_PCS_PROGRAMMING",
            "Error programming WAFL PCS registers",
        ),
        0x73 => {
            ("BL_ERR_WAFL_SET_THRESHOLD", "Error setting WAFL PCS threshold")
        }
        0x74 => ("BL_ERR_LOAD_OEMTRUSTLET", "Error loading OEM trustlets"),
        0x75 => (
            "BL_ERR_RECOVERY_MODE_SYNC",
            "Recovery mode not sync'd across all dies",
        ),
        0x76 => ("BL_ERR_WAFL_UNCORRECTABLE", "Uncorrectable WAFL error"),
        0x77 => ("BL_ERR_MP1_FATAL", "Fatal MP1 error"),
        0x78 => ("BL_ERR_BIOS_RTM_SIG_MISSING", "Failed to find OEM signature"),
        0x79 => ("BL_ERR_BIOS_COPY", "Error copying BIOS to DRAM"),
        0x7A => {
            ("BL_ERR_BIOS_VALIDATION", "Error validating BIOS image signature")
        }
        0x7B => ("BL_ERR_OEM_KEY_INVALID", "OEM Key validation failed"),
        0x7C => (
            "BL_ERR_PLATFORM_BINDING",
            "Platform Vendor/Model ID binding violation",
        ),
        0x7D => (
            "BL_ERR_BIOS_BOOT_FROM_SPI",
            "BIOS boot from SPI-ROM unsupported for PSB",
        ),
        0x7E => (
            "BL_ERR_FUSE_ALREADY_BLOWN",
            "Fuse already blown, reblow will cause ASIC malfunction",
        ),
        0x7F => (
            "BL_ERR_FUSE_ERROR_BLOWNFUSE",
            "Error with actual fusing operation",
        ),
        0x80 => ("BL_ERR_FUSE_INFO_P1", "(P1 socket) Error reading fuse info"),
        0x81 => (
            "BL_ERR_PLATFORM_BINDING_P1",
            "(P1 socket) Platform Vendor/Model ID binding violation",
        ),
        0x82 => {
            ("BL_ERR_FUSE_ALREADY_BLOWN_P1", "(P1 socket) Fuse already blown")
        }
        0x83 => (
            "BL_ERR_FUSE_ERROR_BLOWNFUSE_P1",
            "(P1 socket) Error with fusing operation",
        ),
        0x84 => {
            ("BL_ERR_SEV_ROLLBACK_DETECTED", "SEV FW Rollback attempt detected")
        }
        0x85 => (
            "BL_ERR_SEV_DOWNLOADFW_BROADCAST_FAIL",
            "SEV download FW broadcast failed",
        ),
        0x86 => (
            "BL_ERR_ERROR_INJECTION_NOT_ENABLED",
            "AGESA error injection failure",
        ),
        0x87 => ("BL_ERR_TWIX_UNCORRECTABLE", "Uncorrectable TWIX error"),
        0x88 => {
            ("BL_ERR_TWIX_PROGRAMMING", "Error programming TWIX PCS registers")
        }
        0x89 => {
            ("BL_ERR_TWIX_SET_THRESHOLD", "Error setting TWIX PCS threshold")
        }
        0x8A => ("BL_ERR_CCP_QUEUE_FULL", "SW CCP queue full"),
        0x8B => ("BL_ERR_CCP_CMD_ERROR", "CCP command syntax error"),
        0x8C => ("BL_ERR_CCP_CMD_NOTSCHEDULED", "Command not yet scheduled"),
        0x8D => (
            "BL_ERR_CCP_CMD_BEINGWORKEDON",
            "Command scheduled and being worked on",
        ),
        0x8E => (
            "BL_ERR_DXIO_PHY_SRAM_FW_KEY",
            "DXIO PHY SRAM public key cert loading/auth fails",
        ),
        0x8F => {
            ("BL_ERR_FTPMSIZE_EXCEED_LIMIT", "fTPM binary size exceeds limit")
        }
        0x90 => (
            "BL_ERR_TWIX_LINK_NOT_TRAINED",
            "TWIX link for CCD not trained (fatal)",
        ),
        0x91 => (
            "BL_ERR_MCM_SS_CHECK_FAILED",
            "Security check failed (dies in different security states)",
        ),
        0x92 => (
            "BL_ERR_FWTYPE_MISMATCH",
            "FW type mismatch (requested vs embedded header)",
        ),
        0x93 => (
            "BL_ERR_SVC_CALL_ADDR_VIOLATION",
            "SVC call input parameter address violation",
        ),
        0x94 => {
            ("BL_ERR_FCL_MISMATCH", "Firmware Compatibility Level mismatch")
        }
        0x95 => ("BL_ERR_ESPI_SLAVE_TIMEOUT", "Timeout in eSPI slave device"),
        0x96 => ("BL_ERR_INVALID_IDEVID", "IDEVID certificate is invalid"),
        0x97 => ("BL_ERR_INVALID_MODULE_HEADER", "Invalid header version"),
        0x98 => (
            "BL_ERR_INVALID_MEASUREMENT_ALGORITHM",
            "Invalid or deprecated SHA algorithm",
        ),
        0x99 => {
            ("BL_ERR_KNOLL_KEY_DERIV", "Error during Knoll/Prom key derivation")
        }
        0x9A => {
            ("BL_ERR_CCP_NULL_PTR", "Null pointer passed to Crypto function")
        }
        0x9B => (
            "BL_ERR_PSP_SMU_UNKNOWN_MSG_FAIL",
            "SMU reports Unknown command (non-blocking)",
        ),
        0x9C => {
            ("BL_ERR_KNOLL_INVALID_RESPONSE", "Knoll returned invalid response")
        }
        0x9D => {
            ("BL_ERR_KNOLL_SEND_CMD_FAILED", "Failed in Knoll Send Command")
        }
        0x9E => {
            ("BL_ERR_KNOLL_TEST_FAILED", "No Knoll device found (MAC verify)")
        }
        0x9F => {
            ("BL_ERR_POSTCODE_MAX_VALUE", "Maximum allowable error post code")
        }
        // Progress/Success codes 0xA0-0xFF
        0xA0 => ("BL_SUCCESS_C_MAIN", "Successfully entered C Main"),
        0xA1 => {
            ("BL_SUCCESS_P2PMBOX_INIT", "Master initialized C2P / slave waited")
        }
        0xA2 => ("BL_SUCCESS_DERIVE_HMAC_KEY", "HMAC key derived"),
        0xA3 => (
            "BL_SUCCESS_DETECT_BOOT_MODE",
            "Boot Mode detected and sent to slaves",
        ),
        0xA4 => ("BL_SUCCESS_SPI_INIT", "SpiRom initialized"),
        0xA5 => (
            "BL_SUCCESS_COPY_BIOS_DIRECTORY",
            "BIOS Directory read from SPI to SRAM",
        ),
        0xA6 => ("BL_SUCCESS_CHECK_EARLY_UNLOCK", "Early unlock check"),
        0xA7 => ("BL_SUCCESS_DERIVE_INLINE_AES_KEY", "Inline AES key derived"),
        0xA8 => (
            "BL_SUCCESS_INLINE_AES_KEY_PROGRAM",
            "Inline-AES key programming done",
        ),
        0xA9 => (
            "BL_SUCCESS_INLINE_AES_KEY_WRAPPER",
            "Inline-AES key wrapper derivation done",
        ),
        0xAA => ("BL_SUCCESS_IP_CONFIG", "Loaded HW IP configuration values"),
        0xAB => ("BL_SUCCESS_MBAT_PROGRAMMING", "MBAT table programmed"),
        0xAC => ("BL_SUCCESS_LOAD_SMU", "SMU FW loaded"),
        0xAD => ("BL_SUCCESS_SET_RECOVERY_MODE", "Entering Recovery Mode"),
        0xAE => {
            ("BL_SUCCESS_USER_MODE_TEST_UAPP", "User mode test Uapp completed")
        }
        0xAF => ("BL_SUCCESS_START_AGESA", "Loaded Agesa0 from SpiRom"),
        0xB0 => ("BL_SUCCESS_FINISHED_AGESA", "AGESA phase completed"),
        0xB1 => {
            ("BL_SUCCESS_POST_DRAM_TESTS", "Post DRAM training tests completed")
        }
        0xB2 => {
            ("BL_SUCCESS_IDEVID_VALIDATION", "IDEVID validation successful")
        }
        0xB3 => (
            "BL_SUCCESS_BYPASS_IDEVID_CHECK",
            "IDEVID validation failed but bypassed (unsecure)",
        ),
        0xB4 => (
            "BL_SUCCESS_RUN_SECURITY_GASKET",
            "Security Gasket binary validated and run",
        ),
        0xB5 => (
            "BL_SUCCESS_UMC_SECURITY_INIT",
            "UMC Keys generated and programmed",
        ),
        0xB6 => (
            "BL_SUCCESS_STORE_WRAPPED_KEY",
            "Inline AES key wrapper stored in DRAM",
        ),
        0xB7 => (
            "BL_SUCCESS_VALIDATED_OEM_KEY",
            "FW Validation step completed (OEM key)",
        ),
        0xB8 => (
            "BL_SUCCESS_VALIDATED_BIOS_RST",
            "FW Validation step completed (BIOS RST)",
        ),
        0xB9 => (
            "BL_SUCCESS_LOADING_BIOS_COMPONENTS",
            "BIOS copy from SPI to DRAM complete",
        ),
        0xBA => {
            ("BL_SUCCESS_VALIDATED_BIOS", "FW Validation step completed (BIOS)")
        }
        0xBB => (
            "BL_SUCCESS_BIOS_LOAD_COMPLETE",
            "BIOS load process fully complete",
        ),
        0xBC => ("BL_SUCCESS_RELEASE_X86", "x86 released"),
        0xBD => ("BL_SUCCESS_NORMAL_UNLOCK", "Early Secure Debug completed"),
        0xBE => {
            ("BL_SUCCESS_GET_VERSION_COMMAND", "GetFWVersion command completed")
        }
        0xBF => ("BL_SUCCESS_SMI_INFO_COMMAND", "SMIInfo command completed"),
        0xC0 => ("BL_SUCCESS_ENTER_WARM_BOOT", "Entered WarmBootResume()"),
        0xC1 => (
            "BL_SUCCESS_COPIED_SECURE_OS_SRAM",
            "SecureOS image copied to SRAM",
        ),
        0xC2 => (
            "BL_SUCCESS_COPIED_TRUSTLETS_DRAM",
            "Trustlets copied to PSP Secure Memory",
        ),
        0xC3 => {
            ("BL_SUCCESS_JUMPING_TO_SECURE_OS", "About to jump to Secure OS")
        }
        0xC4 => (
            "BL_SUCCESS_RESTORED_CCP_STATE",
            "CCP and UMC state restored on S3 resume",
        ),
        0xC5 => (
            "BL_SUCCESS_WARM_MB_SRAMHMAC_PASS",
            "PSP SRAM HMAC validated by Mini BL",
        ),
        0xC6 => (
            "BL_SUCCESS_WARM_MB_TRANSFER2OS",
            "About to jump to t-base in Mini BL",
        ),
        0xC7 => (
            "BL_SUCCESS_VMG_ECDH_UNIT_TEST_START",
            "VMG ECDH unit test started",
        ),
        0xC8 => {
            ("BL_SUCCESS_VMG_ECDH_UNIT_TEST_PASS", "VMG ECDH unit test passed")
        }
        0xC9 => (
            "BL_SUCCESS_VMG_ECC_CDH_TEST_START",
            "VMG ECC CDH unit test started",
        ),
        0xCA => {
            ("BL_SUCCESS_VMG_ECC_CDH_TEST_PASS", "VMG ECC CDH unit test passed")
        }
        0xCB => (
            "BL_SUCCESS_VMG_KDF_TEST_START",
            "VMG KDF-CTR HMAC unit test started",
        ),
        0xCC => (
            "BL_SUCCESS_VMG_KDF_TEST_PASS",
            "VMG KDF-CTR HMAC unit test passed",
        ),
        0xCD => {
            ("BL_SUCCESS_VMG_LAUNCH_TEST_START", "VMG LAUNCH_* test started")
        }
        0xCE => ("BL_SUCCESS_VMG_LAUNCH_TEST_PASS", "VMG LAUNCH_* test passed"),
        0xCF => (
            "BL_SUCCESS_MP1_RESET_COMPLETE",
            "MP1 out of reset, executing SMUFW",
        ),
        0xD0 => (
            "BL_SUCCESS_PSP_SMU_RESERVED_PROG",
            "PSP and SMU Reserved Addresses correct",
        ),
        0xD1 => {
            ("BL_SUCCESS_PSP_STEADY_STATE", "Reached steady-state WFI loop")
        }
        0xD2 => ("BL_SUCCESS_WDT_1_COUNTER_EXPIRED", "WDT Stage 1 Expiry"),
        0xD3 => {
            ("BL_SUCCESS_KNOLL_NONCE_COMPLETE", "32-byte RandOut from Knoll")
        }
        0xD4 => ("BL_SUCCESS_KNOLL_MAC_COMPLETE", "32-byte MAC from Knoll"),
        0xD5 => ("BL_SUCCESS_KNOLL_VERIFIED", "Knoll device verified"),
        0xD6 => (
            "BL_SUCCESS_CNLI_SECURITY_INIT",
            "CNLI Keys generated and programmed",
        ),
        0xD7 => (
            "BL_RECOVERY_TRUSTLET_VALIDATE_FAIL",
            "Recovery: trustlet validation fail",
        ),
        0xD8 => {
            ("BL_RECOVERY_OS_VALIDATE_FAIL", "Recovery: OS validation fail")
        }
        0xD9 => (
            "BL_RECOVERY_OEM_PUBLIC_KEY_FAIL",
            "Recovery: OEM public key not found",
        ),
        0xDA => {
            ("BL_RECOVERY_HEADER_CORRUPTION", "Recovery: header corruption")
        }
        0xDB => {
            ("BL_ERR_SECURE_UNLOCK_FAIL", "Secure unlock fail (non-blocking)")
        }
        0xDC => (
            "BL_SUCCESS_SRAM_IMAGE_ALREADY_LOADED",
            "FW image already loaded in SRAM",
        ),
        0xE0 => ("BL_SUCCESS_SECURE_UNLOCK_RETURN", "Unlock return"),
        0xE2 => (
            "BL_SUCCESS_TRIGGER_SYSTEM_RESET",
            "Token expiration reset triggered",
        ),
        0xE3 => (
            "BL_SUCCESS_VALIDATED_DXIO_PHY_FW_KEY",
            "DXIO PHY SRAM FW key validated",
        ),
        0xE4 => ("BL_SUCCESS_MP1_SRAM_LOAD", "MP1 firmware loaded to SRAM"),
        0xE5 => ("BL_SUCCESS_MP1_SRAM_READ", "MP1 SRAM read successfully"),
        0xE6 => ("BL_SUCCESS_MP1_RESET_DONE", "MP1 reset successfully"),
        0xE7 => ("BL_SUCCESS_DF_INIT", "DF init done (without AGESA)"),
        0xE8 => ("BL_SUCCESS_UMC_INIT", "UMC init done (without AGESA)"),
        0xE9 => ("BL_SUCCESS_LX6_ROM_READY", "LX6 Boot ROM code ready"),
        0xEA => ("BL_SUCCESS_LX6_ASSERT_RESET", "LX6 reset asserted"),
        0xEB => ("BL_SUCCESS_LX6_SRAM_LOAD", "LX6 loaded to SRAM"),
        0xEC => {
            ("BL_SUCCESS_LX6_RESET_VECTOR_SEL", "LX6 reset vector set to SRAM")
        }
        0xED => ("BL_SUCCESS_LX6_DEASSERT_RESET", "LX6 reset de-asserted"),
        0xEE => ("BL_SUCCESS_LX6_FW_READY", "LX6 firmware running and ready"),
        0xEF => ("BL_SUCCESS_S3_IMAGE_LOAD_DONE", "S3 image loaded"),
        0xF0 => (
            "BL_SUCCESS_2K4K_KEY_VERIFY",
            "Verified signed image using 4K/2K key",
        ),
        0xF1 => {
            ("BL_SUCCESS_MULTI_SOCKET_BOOT", "Multi-socket boot identified")
        }
        0xF2 => (
            "BL_SUCCESS_SECURITY_POLICY_CHECK",
            "Security Policy check successful",
        ),
        0xF3 => ("BL_SUCCESS_SS3", "SS3 loaded"),
        0xF4 => ("BL_SUCCESS_FTPM", "fTPM Driver loaded"),
        0xF5 => ("BL_SUCCESS_SYS_DRV", "sys_drv loaded"),
        0xF6 => ("BL_SUCCESS_SOS", "Secure OS loaded"),
        0xF7 => (
            "BL_SUCCESS_CONTROL_TO_SOS",
            "About to transfer control to secureOS",
        ),
        0xFF => ("BL_SUCCESS_BOOT_DONE", "Bootloader sequence finished"),
        _ => ("(unknown)", "Unknown bootloader status code"),
    }
}

fn look_up_abl(code: u32) -> Option<(&'static str, &'static str)> {
    Some(match code {
        // Db Errors
        0xEA000ACE => ("TpAcgDbErrorStart", "ApcbConfgGet DbError start"),
        0xEA000ACF => ("TpAcgDbErrorEnd", "ApcbConfgGet DbError end"),

        // ABL L0 Test Points
        0xEA00ABC1 => ("TpABL0Code001", "ABL L0 Test Point 001"),
        0xEA00ABC2 => ("TpABL0Code002", "ABL L0 Test Point 002"),
        0xEA00ABC3 => ("TpABL0Code003", "ABL L0 Test Point 003"),
        0xEA00ABC4 => ("TpABL0Code004", "ABL L0 Test Point 004"),
        0xEA00ABC5 => ("TpABL0Code005", "ABL L0 Test Point 005"),
        0xEA00ABC6 => ("TpABL0Code006", "ABL L0 Test Point 006"),
        0xEA00ABC7 => ("TpABL0Code007", "ABL L0 Test Point 007"),
        0xEA00ABC8 => ("TpABL0Code008", "ABL L0 Test Point 008"),
        0xEA00ABC9 => ("TpABL0Code009", "ABL L0 Test Point 009"),
        0xEA00ABCA => ("TpABL0Code00A", "ABL L0 Test Point 00A"),
        0xEA00ABCB => ("TpABL0Code00B", "ABL L0 Test Point 00B"),
        0xEA00ABCC => ("TpABL0Code00C", "ABL L0 Test Point 00C"),
        0xEA00ABCD => ("TpABL0Code00D", "ABL L0 Test Point 00D"),
        0xEA00ABCE => ("TpABL0Code00E", "ABL L0 Test Point 00E"),
        0xEA00ABCF => ("TpABL0Code00F", "ABL L0 Test Point 00F"),

        // ABL L15 Test Points
        0xEA00ABE1 => ("TpABL15Code001", "ABL L15 Test Point 001"),
        0xEA00ABE2 => ("TpABL15Code002", "ABL L15 Test Point 002"),
        0xEA00ABE3 => ("TpABL15Code003", "ABL L15 Test Point 003"),
        0xEA00ABE4 => ("TpABL15Code004", "ABL L15 Test Point 004"),
        0xEA00ABE5 => ("TpABL15Code005", "ABL L15 Test Point 005"),
        0xEA00ABE6 => ("TpABL15Code006", "ABL L15 Test Point 006"),
        0xEA00ABE7 => ("TpABL15Code007", "ABL L15 Test Point 007"),
        0xEA00ABE8 => ("TpABL15Code008", "ABL L15 Test Point 008"),
        0xEA00ABE9 => ("TpABL15Code009", "ABL L15 Test Point 009"),
        0xEA00ABEA => ("TpABL15Code010", "ABL L15 Test Point 010"),
        0xEA00ABEB => ("TpABL15Code011", "ABL L15 Test Point 011"),
        0xEA00ABEC => ("TpABL15Code012", "ABL L15 Test Point 012"),
        0xEA00ABED => ("TpABL15Code013", "ABL L15 Test Point 013"),
        0xEA00ABEE => ("TpABL15Code014", "ABL L15 Test Point 014"),
        0xEA00ABEF => ("TpABL15Code015", "ABL L15 Test Point 015"),
        0xEA00ABF0 => ("TpABL15Code016", "ABL L15 Test Point 016"),
        0xEA00ABF1 => ("TpABL15Code017", "ABL L15 Test Point 017"),
        0xEA00ABF2 => ("TpABL15Code018", "ABL L15 Test Point 018"),
        0xEA00ABF3 => ("TpABL15Code019", "ABL L15 Test Point 019"),
        0xEA00ABF4 => ("TpABL15Code020", "ABL L15 Test Point 020"),

        // Memory & Overclock Errors
        0xEA00BAAB => (
            "TpApcbMemOverclockRecoveryError",
            "Memory Overclock recovery Fail",
        ),
        0xEA00BAAC => {
            ("TpAblMemoryOverclockErrorRRWText", "Memory Overclock RRW Text")
        }
        0xEA00BAAD => ("TpMemEmulationError", "Memory Emulation Fail"),
        0xEA00E2CF => (
            "TpAblErrorMemOverclockErrorRrwTestResults",
            "Over Clock RRW Test Results Error",
        ),

        // Processor Test Points (Memory/PMU)
        0xEA00E001 => (
            "TpProcMemBeforeMemDataInit",
            "Memory structure initialization (Public interface)",
        ),
        0xEA00E002 => (
            "TpProcMemBeforeSpdProcessing",
            "SPD Data processing (Public interface)",
        ),
        0xEA00E003 => (
            "TpProcMemAmdMemAutoPhase1",
            "Memory configuration Phase 1 (Public interface)",
        ),
        0xEA00E004 => ("TpProcMemDramInit", "DRAM initialization"),
        0xEA00E005 => ("TpProcMemSPDChecking", "SPD Checking"),
        0xEA00E006 => ("TpProcMemModeChecking", "Mode Checking"),
        0xEA00E007 => {
            ("TpProcMemSpeedTclConfig", "Speed and TCL configuration")
        }
        0xEA00E008 => ("TpProcMemSpdTiming", "SPD Timing"),
        0xEA00E009 => ("TpProcMemDramMapping", "DRAM Mapping"),
        0xEA00E00A => {
            ("TpProcMemPlatformSpecificConfig", "Platform Specific Config")
        }
        0xEA00E00B => ("TPProcMemPhyCompensation", "ProcMemPhyCompensation"),
        0xEA00E00C => ("TpProcMemStartDcts", "Start DCTs"),
        0xEA00E00D => {
            ("TpProcMemBeforeDramInit", "Before DRAM Init (Public interface)")
        }
        0xEA00E00E => ("TpProcMemPhyFenceTraining", "Phy Fence Training"),
        0xEA00E00F => ("TpProcMemSynchronizeDcts", "Synchronize DCTs"),
        0xEA00E010 => ("TpProcMemSystemMemoryMapping", "System Memory Mapping"),
        0xEA00E011 => ("TpProcMemMtrrConfiguration", "MTRR Configuration"),
        0xEA00E012 => ("TpProcMemDramTraining", "DRAM Training"),
        0xEA00E013 => (
            "TpProcMemBeforeAnyTraining",
            "Before Any Training (Public interface)",
        ),
        0xEA00E014 => {
            ("TpProcMemPmuBeforeFirmwareLoad", "PMU - Before Firmware load")
        }
        0xEA00E015 => {
            ("TpProcMemPmuAfterFirmwareLoad", "PMU - After Firmware load")
        }
        0xEA00E016 => {
            ("TpProcMemPmuPopulateSramTimings", "PMU Populate SRAM Timing")
        }
        0xEA00E017 => {
            ("TpProcMemPmuPopulateSramConfig", "PMU Populate SRAM Config")
        }
        0xEA00E018 => {
            ("TpProcMemPmuWriteSramMsgBlock", "PMU Write SRAM Msg Block")
        }
        0xEA00E019 => {
            ("TpProcMemPmuWaitForPhyCalComplete", "Wait for Phy Cal Complete")
        }
        0xEA00E01A => ("TpProcMemPmuPhyCalComplete", "Phy Cal Complete"),
        0xEA00E01B => ("TpProcMemPmuStart", "PMU Start"),
        0xEA00E01C => ("TpProcMemPmuStarted", "PMU Started"),
        0xEA00E01D => {
            ("TpProcMemPmuWaitingForComplete", "PMU Waiting for Complete")
        }
        0xEA00E01E => ("TpProcMemPmuStageDevInit", "PMU Stage Dev Init"),
        0xEA00E01F => {
            ("TpProcMemPmuStageTrainWrLvl", "PMU Stage Training Wr Lvl")
        }
        0xEA00E020 => {
            ("TpProcMemPmuStageTrainRxEn", "PMU Stage Training Rx En")
        }
        0xEA00E021 => {
            ("TpProcMemPmuStageTrainRdDqs1D", "PMU Stage Training Rd Dqs 1D")
        }
        0xEA00E022 => {
            ("TpProcMemPmuStageTrainRd2D", "PMU Stage Training Rd 2D")
        }
        0xEA00E023 => {
            ("TpProcMemPmuStageTrainWr2D", "PMU Stage Training Wr 2D")
        }
        0xEA00E024 => ("TpProcMemPmuStagePMUQEmpty", "PMU Queue Empty"),
        0xEA00E025 => ("TpProcMemPmuUSMsgStart", "PMU US message Start"),
        0xEA00E026 => ("TpProcMemPmuUSMsgEnd", "PMU US message End"),
        0xEA00E027 => ("TpProcMemPmuComplete", "PMU Complete"),
        0xEA00E028 => ("TpProcMemAfterPmuTraining", "After PMU Training"),
        0xEA00E029 => ("TpProcMemBeforeDisablePmu", "Before Disable PMU"),
        0xEA00E02A => ("TpProcMemTransmitDqsTraining", "Transmit DQS Training"),
        0xEA00E02B => ("TpProcMemTxDqStartSweep", "Start write sweep"),
        0xEA00E02C => ("TpProcMemTxDqSetDelay", "Set Transmit DQ delay"),
        0xEA00E02D => ("TpProcMemTxDqWritePattern", "Write test pattern"),
        0xEA00E02E => ("TpProcMemTxDqReadPattern", "Read test pattern"),
        0xEA00E02F => ("TpProcMemTxDqTestPattern", "Compare test pattern"),
        0xEA00E030 => ("TpProcMemTxDqResults", "Update results"),
        0xEA00E031 => ("TpProcMemTxDqFindWindow", "Start find passing window"),
        0xEA00E032 => {
            ("TpProcMemMaxRdLatencyTraining", "Max Rd Latency Training")
        }
        0xEA00E033 => ("TpProcMemMaxRdLatStartSweep", "Max Rd Lat Start sweep"),
        0xEA00E034 => ("TpProcMemMaxRdLatSetDelay", "Max Rd Lat Set delay"),
        0xEA00E035 => {
            ("TpProcMemMaxRdLatWritePattern", "Max Rd Lat Write test pattern")
        }
        0xEA00E036 => {
            ("TpProcMemMaxRdLatReadPattern", "Max Rd Lat Read test pattern")
        }
        0xEA00E037 => {
            ("TpProcMemMaxRdLatTestPattern", "Max Rd Lat Compare test pattern")
        }
        0xEA00E038 => ("TpProcMemOnlineSpareInit", "Online Spare init"),
        0xEA00E039 => {
            ("TpProcMemChipSelectInterleaveInit", "Chip Select Interleave Init")
        }
        0xEA00E03A => ("TpProcMemNodeInterleaveInit", "Node Interleave Init"),
        0xEA00E03B => {
            ("TpProcMemChannelInterleaveInit", "Channel Interleave Init")
        }
        0xEA00E03C => ("TpProcMemEccInitialization", "ECC initialization"),
        0xEA00E03D => {
            ("TpProcMemPlatformSpecificInit", "Platform Specific Init")
        }
        0xEA00E03E => {
            ("TpProcMemBeforeAgesaReadSpd", "Before callout AgesaReadSpd")
        }
        0xEA00E03F => {
            ("TpProcMemAfterAgesaReadSpd", "After callout AgesaReadSpd")
        }
        0xEA00E040 => (
            "TpProcMemBeforeAgesaHookBeforeDramInit",
            "Before callout AgesaHookBeforeDramInit",
        ),
        0xEA00E041 => (
            "TpProcMemAfterAgesaHookBeforeDramInit",
            "After callout AgesaHookBeforeDramInit",
        ),
        0xEA00E042 => (
            "TpProcMemBeforeAgesaHookBeforeDQSTraining",
            "Before callout AgesaHookBeforeDQSTraining",
        ),
        0xEA00E043 => (
            "TpProcMemAfterAgesaHookBeforeDQSTraining",
            "After callout AgesaHookBeforeDQSTraining",
        ),
        0xEA00E044 => (
            "TpProcMemBeforeAgesaHookBeforeExitSelfRef",
            "Before callout AgesaHookBeforeExitSelfRef",
        ),
        0xEA00E045 => (
            "TpProcMemAfterAgesaHookBeforeExitSelfRef",
            "After callout AgesaHookBeforeExitSelfRef",
        ),
        0xEA00E046 => ("TpProcMemAfterMemDataInit", "After MemDataInit"),
        0xEA00E047 => ("TpProcMemInitializeMCT", "Before InitializeMCT"),
        0xEA00E048 => ("TpProcMemLvDdr3", "Before LV DDR3"),
        0xEA00E049 => ("TpProcMemInitMCT", "Before InitMCT"),
        0xEA00E04A => ("TpProcMemOtherTiming", "Before OtherTiming"),
        0xEA00E04B => ("TpProcMemUMAMemTyping", "Before UMAMemTyping"),
        0xEA00E04C => ("TpProcMemSetDqsEccTmgs", "Before SetDqsEccTmgs"),
        0xEA00E04D => ("TpProcMemMemClr", "Before MemClr"),
        0xEA00E04E => ("TpProcMemOnDimmThermal", "Before On DIMM Thermal"),
        0xEA00E04F => ("TpProcMemDmi", "Before DMI"),
        0xEA00E050 => ("TpProcMemEnd", "End of phase 3 memory code"),
        0xEA00E080 => ("TpProcMemSendMRS2", "Sending MRS2"),
        0xEA00E081 => ("TpProcMemSendMRS3", "Sending MRS3"),
        0xEA00E082 => ("TpProcMemSendMRS1", "Sending MRS1"),
        0xEA00E083 => ("TpProcMemSendMRS0", "Sending MRS0"),
        0xEA00E084 => {
            ("TpProcMemContinPatternGenRead", "Continuous Pattern Read")
        }
        0xEA00E085 => {
            ("TpProcMemContinPatternGenWrite", "Continuous Pattern Write")
        }
        0xEA00E086 => ("TpProcMem2dRdDqsTraining", "2D RdDqs Training begin"),
        0xEA00E087 => (
            "TpProcMemBefore2dTrainExtVrefChange",
            "Before 2D Training External Vref change",
        ),
        0xEA00E088 => (
            "TpProcMemAfter2dTrainExtVrefChange",
            "After 2D Training External Vref change",
        ),
        0xEA00E089 => {
            ("TpProcMemConfigureDCTForGeneral", "Configure DCT for General use")
        }
        0xEA00E08A => (
            "TpProcMemProcConfigureDCTForTraining",
            "Configure DCT for training",
        ),
        0xEA00E08B => (
            "TpProcMemConfigureDCTNonExplicitSeq",
            "Configure DCT for Non-Explicit",
        ),
        0xEA00E08C => ("TpProcMemSynchronizeChannels", "Sync channels"),
        0xEA00E08D => ("TpProcMemC6StorageAllocation", "Allocate C6 Storage"),
        0xEA00E08E => ("TpProcMemLvDdr4", "Before LV DDR4"),
        0xEA00E08F => ("TpProcMemLvLpddr3", "Before LV LPDDR3"),

        // GNB Early Init
        0xEA00E090 => ("TP0x90", "TP0x90"),
        0xEA00E091 => ("TP0x91", "GNB earlier interface"),
        0xEA00E092 => ("TP0x92", "GNB Early VGA entry"),
        0xEA00E093 => ("TP0x93", "GNB Early VGA exit"),
        0xEA00E094 => ("TP0x94", "GNB Initialization entry"),
        0xEA00E095 => ("TP0x95", "GNB Initialization exit"),
        0xEA00E096 => ("TP0x96", "GNB internal debug code"),
        0xEA00E097 => ("TP0x97", "GNB internal debug code"),
        0xEA00E098 => ("TP0x98", "GNB internal debug code"),
        0xEA00E099 => ("TP0x99", "GNB internal debug code"),
        0xEA00E09A => ("TP0x9A", "GNB internal debug code"),
        0xEA00E09B => ("TP0x9B", "GNB internal debug code"),
        0xEA00E09C => ("TP0x9C", "GNB internal debug code"),
        0xEA00E09D => ("TP0x9D", "GNB internal debug code"),
        0xEA00E09E => ("TP0x9E", "GNB internal debug code"),
        0xEA00E09F => ("TP0x9F", "GNB internal debug code"),
        0xEA00E0A0 => ("TP0xA0", "TP0xA0"),
        0xEA00E0A1 => ("TP0xA1", "GNB internal debug code"),
        0xEA00E0A2 => ("TP0xA2", "GNB internal debug code"),
        0xEA00E0A3 => ("TP0xA3", "GNB internal debug code"),
        0xEA00E0A4 => ("TP0xA4", "GNB internal debug code"),
        0xEA00E0A5 => ("TP0xA5", "GNB internal debug code"),
        0xEA00E0A6 => ("TP0xA6", "GNB internal debug code"),
        0xEA00E0A7 => ("TP0xA7", "GNB internal debug code"),
        0xEA00E0A8 => ("TP0xA8", "GNB internal debug code"),
        0xEA00E0A9 => ("TP0xA9", "GNB internal debug code"),
        0xEA00E0AA => ("TP0xAA", "GNB internal debug code"),
        0xEA00E0AB => ("TP0xAB", "GNB internal debug code"),
        0xEA00E0AC => ("TP0xAC", "GNB internal debug code"),
        0xEA00E0AD => ("TP0xAD", "GNB internal debug code"),
        0xEA00E0AE => ("TP0xAE", "GNB internal debug code"),
        0xEA00E0AF => ("TP0xAF", "GNB internal debug code"),

        0xEA00E0D1 => {
            ("TpProcMemAmdMemAutoPhase2", "ABL 2 memory initialization")
        }
        0xEA00E0D2 => {
            ("TpProcMemAmdMemAutoPhase3", "ABL 3 memory initialization")
        }
        0xEA00E0F9 => ("TpProcMemPmuFailed", "Failed PMU training"),
        0xEA00E0FA => ("TpProcMemPhase1End", "End of phase 1 memory code"),
        0xEA00E0FB => ("TpProcMemPhase2End", "End of phase 2 memory code"),
        0xEA00E104 => ("TpProcMemPhase1bEnd", "End of phase 1b memory code"),
        0xEA00E105 => {
            ("TpProcMemAmdMemAutoPhase1b", "ABL 1b memory initialization")
        }

        // CPU Test Points
        0xEA00E051 => {
            ("TpProcCpuInitAfterTrainingStart", "Entry CPU init after training")
        }
        0xEA00E052 => {
            ("TpProcCpuInitAfterTrainingEnd", "Exit CPU init after training")
        }
        0xEA00E053 => ("TpProcCpuApobInitStart", "Entry CPU APOB data init"),
        0xEA00E054 => ("TpProcCpuApobInitEnd", "Exit CPU APOB data init"),
        0xEA00E055 => {
            ("TpProcCpuOptimizedBootStart", "Entry CPU Optimized boot init")
        }
        0xEA00E056 => {
            ("TpProcCpuOptimizedBootEnd", "Exit CPU Optimized boot init")
        }
        0xEA00E057 => {
            ("TpProcCpuApobCcxEdcInitStart", "Entry CPU APOB EDC info init")
        }
        0xEA00E058 => {
            ("TpProcCpuApobCcxEdcInitEnd", "Exit CPU APOB EDC info init")
        }
        0xEA00E059 => {
            ("TpProcCpuApobCcdMapStart", "Entry CPU APOB CCD map data init")
        }
        0xEA00E05A => {
            ("TpProcCpuApobCcdMapEnd", "Exit CPU APOB CCD map data init")
        }

        // Miscellaneous
        0xEA00EA00 => ("TpAblBegin", "ABL Begin"),
        0xEA00EA01 => ("TpAblEnd", "ABL End"),
        0xEA00EA10 => ("TpAblDebugSync", "ABL Debug Synchronization"),
        0xEA00E0FC => ("TpAbl0Begin", "ABL 0 Begin"),
        0xEA00E0FD => ("TpAbl0End", "ABL 0 End"),
        0xEA00E0FE => ("TpAbl0FatalBegin", "ABL 0 Begin with Fatal Mode"),
        0xEA00E0FF => ("TpAbl0FatalEnd", "ABL 0 End with Fatal Mode"),
        0xEA00E2A0 => ("TpAblErroGeneralAssert", "ABL Error General ASSERT"),
        0xEA00E323 => {
            ("TpAblMbistDefaultRrwTest", "Memory MBIST RRW default test")
        }
        0xEA00E324 => {
            ("TpAblMemoryMbistInterfaceTest", "Memory MBIST Interface test")
        }
        0xEA00E325 => ("TpAblMemoryMbistDataEyeTest", "Memory MBIST DataEye"),
        0xEA00E326 => {
            ("TpAblMemoryPostPackageRepair", "Memory Post Package Repair")
        }
        0xEA00E336 => ("TpAblMemoryHealStart", "Memory Heal BIST Start"),
        0xEA00E337 => ("TpAblMemoryHealEnd", "Memory Heal BIST End"),
        0xEA00E338 => ("TpAblMemoryHealWrite", "Memory Heal BIST Write"),
        0xEA00E339 => ("TpAblMemoryHealRead", "Memory Heal BIST Read"),
        0xEA00E33A => ("TpAblMemoryHealRepair", "Memory Heal BIST Repair"),
        0xEA00E33B => (
            "TpAblTimeoutAtSwitchToMemoryTrainingState",
            "Timeout at PMFW SwitchToMemoryTrainingState",
        ),
        0xEA00E33D => {
            ("TpAblMemoryDdrTrainingComplete", "DDR PMU training complete")
        }
        0xEA00E33E => (
            "TpAblTimeoutAtSwitchToStartupDfPstate",
            "Timeout at PMFW SwitchToStartupDfPstate",
        ),
        0xEA00E340 => ("TpAblMemoryDdrTrainingStart", "DDR PMU training start"),
        0xEA00E341 => (
            "TpAblMemoryDdrTrainingPsateStart",
            "DDR PMU training for P-State Start",
        ),
        0xEA00E342 => (
            "TpAblMemoryDdrTrainingPsateEnd",
            "DDR PMU training for P-State End",
        ),
        0xEA00E343 => (
            "TpAblMemoryDdrSpdVerifyCrcFailure",
            "DDR DIMM SPD verify CRC failure",
        ),
        0xEA00E344 => (
            "TpAblMemoryDdrSpdInvalidFieldValue",
            "DDR DIMM SPD Invalid Field Value",
        ),
        0xEA00E350 => (
            "TpAblMemoryDdrRuntimePostPackageRepairBegin",
            "DDR Runtime Post Package Repair Begin",
        ),
        0xEA00E351 => (
            "TpAblMemoryDdrRuntimePostPackageRepairEnd",
            "DDR Runtime Post Package Repair End",
        ),
        0xEA00E352 => ("TpAblMemMORStart", "MOR Start"),
        0xEA00E353 => ("TpAblMemMOREnd", "MOR End"),
        0xEA00E354 => ("TpAblMemMORExecuted", "MOR Executed"),
        0xEA00EFFF => ("EndAgesaTps", "EndAgesas"),

        // ABL1 Codes
        0xEA00E0B0 => ("TpAbl1Begin", "ABL 1 Begin"),
        0xEA00E0B1 => ("TpAbl1Initialization", "ABL 1 Initialization"),
        0xEA00E0B2 => ("TpAbl1DfEarly", "ABL 1 DF Early"),
        0xEA00E0B3 => ("TpAbl1DfPreTraining", "ABL 1 DF Pre Training"),
        0xEA00E0B4 => ("TpAbl1DebugSync", "ABL 1 Debug Synchronization"),
        0xEA00E0B5 => ("TpAbl1ErrorDetected", "ABL 1 Error Detected"),
        0xEA00E0B6 => (
            "TpAbl1GlobalMemErrorDetected",
            "ABL 1 Global memory error detected",
        ),
        0xEA00E0B7 => ("TpAbl1End", "ABL 1 End"),
        0xEA00E0D4 => ("TpAbl1EnterMemFlow", "ABL 1 Enter Memory Flow"),
        0xEA00E0D5 => (
            "TpAbl1MemFlowMemClkSync",
            "Memory flow memory clock synchronization",
        ),
        0xEA00E107 => ("TpAbl1bDebugSync", "ABL 1b Debug Synchronization"),
        0xEA00E109 => ("TpAbl1bBegin", "ABL 1b Begin"),
        0xEA00E10C => ("TpAbl18End", "ABL 18 End"),
        0xEA00E10D => ("TpAbl18ResumeInitialization", "ABL 18 Resume boot"),
        0xEA00E10E => ("TpAbl15End", "ABL 15 End"),
        0xEA00E10F => ("TpAbl15Initialization", "ABL 15 Initialization"),

        // ABL2 Codes
        0xEA00E0B8 => ("TpAbl2Begin", "ABL 2 Begin"),
        0xEA00E0B9 => ("TpAbl2Initialization", "ABL 2 Initialization"),
        0xEA00E0BA => ("TpAbl2DfAfterTraining", "ABL 2 After Training"),
        0xEA00E0BB => ("TpAbl2DebugSync", "ABL 2 Debug Synchronization"),
        0xEA00E0BC => ("TpAbl2ErrorDetected", "ABL 2 Error detected"),
        0xEA00E0BD => (
            "TpAbl2GlobalMemErrorDetected",
            "ABL 2 Global memory error detected",
        ),
        0xEA00E0BE => ("TpAbl2End", "ABL 2 End"),

        // ABL3 Codes
        0xEA00E0BF => ("TpAbl3Begin", "ABL 3 Begin"),
        0xEA00E0C0 => ("TpAbl3Initialization", "ABL 3 Initialization"),
        0xEA00E1C0 => ("TpAbl3GmiGopInitStage1", "ABL 3 GMI/xGMI Init Stage 1"),
        0xEA00B1C0 => (
            "TpAbl3GmiGopInitStage1Warning",
            "ABL 3 GMI/xGMI Init Stage 1 Warning",
        ),
        0xEA00F1C0 => {
            ("TpAbl3GmiGopInitState1Error", "ABL 3 GMI/xGMI Init Stage 1 Error")
        }
        0xEA00E2C0 => ("TpAbl3GmiGopInitStage2", "ABL 3 GMI/xGMI Init Stage 2"),
        0xEA00B2C0 => (
            "TpAbl3GmiGopInitStage2Warning",
            "ABL 3 GMI/xGMI Init Stage 2 Warning",
        ),
        0xEA00F2C0 => {
            ("TpAbl3GmiGopInitState2Error", "ABL 3 GMI/xGMI Init Stage 2 Error")
        }
        0xEA00E3C0 => ("TpAbl3GmiGopInitStage3", "ABL 3 GMI/xGMI Init Stage 3"),
        0xEA00B3C0 => (
            "TpAbl3GmiGopInitStage3Warning",
            "ABL 3 GMI/xGMI Init Stage 3 Warning",
        ),
        0xEA00F3C0 => {
            ("TpAbl3GmiGopInitState3Error", "ABL 3 GMI/xGMI Init Stage 3 Error")
        }
        0xEA00E4C0 => ("TpAbl3GmiGopInitStage4", "ABL 3 GMI/xGMI Init Stage 4"),
        0xEA00B4C0 => (
            "TpAbl3GmiGopInitStage4Warning",
            "ABL 3 GMI/xGMI Init Stage 4 Warning",
        ),
        0xEA00F4C0 => {
            ("TpAbl3GmiGopInitState4Error", "ABL 3 GMI/xGMI Init Stage 4 Error")
        }
        0xEA00E5C0 => ("TpAbl3GmiGopInitStage5", "ABL 3 GMI/xGMI Init Stage 5"),
        0xEA00B5C0 => (
            "TpAbl3GmiGopInitStage5Warning",
            "ABL 3 GMI/xGMI Init Stage 5 Warning",
        ),
        0xEA00F5C0 => {
            ("TpAbl3GmiGopInitState5Error", "ABL 3 GMI/xGMI Init Stage 5 Error")
        }
        0xEA00E6C0 => ("TpAbl3GmiGopInitStage6", "ABL 3 GMI/xGMI Init Stage 6"),
        0xEA00B6C0 => (
            "TpAbl3GmiGopInitStage6Warning",
            "ABL 3 GMI/xGMI Init Stage 6 Warning",
        ),
        0xEA00F6C0 => {
            ("TpAbl3GmiGopInitState6Error", "ABL 3 GMI/xGMI Init Stage 6 Error")
        }
        0xEA00E7C0 => ("TpAbl3GmiGopInitStage7", "ABL 3 GMI/xGMI Init Stage 7"),
        0xEA00E8C0 => ("TpAbl3GmiGopInitStage8", "ABL 3 GMI/xGMI Init Stage 8"),
        0xEA00E9C0 => ("TpAbl3GmiGopInitStage9", "ABL 3 GMI/xGMI Init Stage 9"),
        0xEA00F9C0 => {
            ("TpAbl3GmiGopInitStage9Error", "ABL 3 GMI/xGMI Init Stage 9 Error")
        }
        0xEA00EAC0 => {
            ("TpAbl3GmiGopInitStage10", "ABL 3 GMI/xGMI Init Stage 10")
        }
        0xEA00FAC0 => (
            "TpAbl3GmiGopInitStage10Error",
            "ABL 3 GMI/xGMI Init Stage 10 Error",
        ),
        0xEA00E0C1 => ("TpAbl3ProgramUmcKeys", "ABL 3 Program UMC Keys"),
        0xEA00E0C2 => {
            ("TpAbl3DfFinalInitialization", "ABL 3 DF Final Initialization")
        }
        0xEA00E0C3 => {
            ("TpAbl3ExecuteSyncFunction", "ABL 3 Execute Sync Function")
        }
        0xEA00E0C4 => ("TpAbl3DebugSync", "ABL 3 Debug Synchronization"),
        0xEA00E0C5 => ("TpAbl3ErrorDetected", "ABL 3 Error Detected"),
        0xEA00E0C6 => (
            "TpAbl3GlobalMemErrorDetected",
            "ABL 3 Global memory error detected",
        ),
        0xEA00E0D3 => ("TpAbl3End", "ABL 3 End"),

        // ABL4 Codes
        0xEA00E0C7 => ("TpAbl4ColdInitialization", "ABL 4 Init - cold boot"),
        0xEA00E0C8 => ("TpAbl4MemTest", "ABL 4 Memory test - cold boot"),
        0xEA00E0C9 => ("TpAbl4Apob", "ABL 4 APOB Init - cold boot"),
        0xEA00E0CA => {
            ("TpAbl4Finalize", "ABL 4 Finalize memory settings - cold boot")
        }
        0xEA00E0CB => (
            "TpAbl4CpuInizialOptimizedBoot",
            "ABL 4 CPU Init Optimized Boot - cold boot",
        ),
        0xEA00E0CC => {
            ("TpAbl4GmicieTraining", "ABL 4 GMI/PCIe Training - cold boot")
        }
        0xEA00E0CD => ("TpAbl4ColdEnd", "ABL 4 Cold boot End"),
        0xEA00E0CE => {
            ("TpAbl4ResumeInitialization", "ABL 4 Init - Resume boot")
        }
        0xEA00E0CF => ("TpAbl4ResumeEnd", "ABL 4 Resume End"),
        0xEA00E0D0 => ("TpAbl4End", "ABL 4 End Cold/Resume boot"),
        0xEA00E108 => ("TpAbl4bDebugSync", "ABL 4b Debug Synchronization"),
        0xEA00E10A => ("TpAbl4bBegin", "ABL 4b Begin"),

        // IDS Interface Callouts
        0xEA00E0E0 => {
            ("TpIfBeforeGetIdsData", "Before IDS callout to get IDS data")
        }
        0xEA00E0E1 => {
            ("TpIfAfterGetIdsData", "After IDS callout to get IDS data")
        }

        // ABL6 Codes
        0xEA00E102 => ("TpAbl6End", "ABL 6 End"),
        0xEA00E103 => ("TpAbl6Initialization", "ABL 6 Initialization"),
        0xEA00E106 => (
            "TpAbl6GlobalMemErrorDetected",
            "ABL 6 Global memory error detected",
        ),

        // ABL7 Codes
        0xEA00E0E2 => ("TpAbl7DebugSync", "ABL 7 Debug Synchronization"),
        0xEA00E100 => ("TpAbl7End", "ABL 7 End"),
        0xEA00E101 => ("TpAbl7ResumeInitialization", "ABL 7 Resume boot"),

        // APOB & CPU Misc
        0xEA00E10B => {
            ("TpProcApobHmacFailOnS3", "BSP HMAC fail on APOB Header")
        }
        0xEA00E110 => {
            ("TpProcBeforeUmcBasedDeviceInit", "Before UMC based device init")
        }
        0xEA00E111 => {
            ("TpProcAfterUmcBasedDeviceInit", "After UMC based device init")
        }
        0xEA00E112 => ("TpProcCcxDowncoreEntry", "CCX Downcore Entry"),
        0xEA00E113 => ("TpProcCcxDowncoreExit", "CCX Downcore Exit"),

        // Error & Event Codes
        0xEA00E2A1 => ("TpAblErrorUnknown", "Unknown Error"),
        0xEA00E2A3 => ("TpAblErrorLogInitError", "Error Log Init Error"),
        0xEA00E2A4 => {
            ("TpAblErrorOdtHeap", "On DIMM Thermal Heap allocation error")
        }
        0xEA00E2A5 => ("TpAblErrorMemoryTest", "Memory test error"),
        0xEA00E2A6 => {
            ("TpAblErrorExecutingMemoryTest", "Error executing memory test")
        }
        0xEA00E2A7 => (
            "TpAblErrorDpprMemAutoHeapAlocError",
            "DDR Post Package Repair Heap Alloc error",
        ),
        0xEA00E2A8 => (
            "TpAblErrorDpprNoApobHeapAlocError",
            "DDR Post Package Repair APOB Heap Alloc error",
        ),
        0xEA00E2A9 => (
            "TpAblErrorDpprNoPprTblHeapAlocError",
            "DDR Post Package Repair PPR Table Heap Alloc error",
        ),
        0xEA00E2AA => {
            ("TpAblErrorEccMemAutoHeapAlocError", "ECC Mem Auto Alloc error")
        }
        0xEA00E2AB => {
            ("TpAblErrorSocScanHeapAlocError", "SoC Scan Heap Alloc error")
        }
        0xEA00E2AC => ("TpAblErrorSocScanNoDieError", "SoC Scan No Die error"),
        0xEA00E2AD => {
            ("TpAblErrorNbTecHeapAlocError", "NB Tech Heap Alloc error")
        }
        0xEA00E2AE => ("TpAblErrorNoNbConstError", "No NB Constructor error"),
        0xEA00E2B0 => {
            ("TpAblErrorNoTechConstError", "No Tech Constructor error")
        }
        0xEA00E2B3 => {
            ("TpAblErrorAbl2NoNbConst", "ABL2 No NB Constructor error")
        }
        0xEA00E2B4 => ("TpAblErrorAbl3AutoAloc", "ABL3 Auto Allocation error"),
        0xEA00E2B5 => {
            ("TpAblErrorAbl3NoNbConst", "ABL3 No NB Constructor error")
        }
        0xEA00E2B7 => ("TpAblErrorAbl2Gen", "ABL2 General error"),
        0xEA00E2B8 => ("TpAblErrorAbl3Gen", "ABL3 General error"),
        0xEA00E2B9 => ("TpAblErrorGetTargetSpeed", "Get Target Speed error"),
        0xEA00E2BA => {
            ("TpAblErrorFlowP1FamilySupport", "Flow P1 Family Support error")
        }
        0xEA00E2BB => {
            ("TpAblErrorNoValidDdr4Dimms", "No Valid DDR4 DIMMs error")
        }
        0xEA00E2BC => ("TpAblErrorNoDimmPresent", "No DIMM Present error"),
        0xEA00E2BD => {
            ("TpAblErrorFlowP2FamilySupport", "Flow P2 Family Support error")
        }
        0xEA00E2BE => (
            "TpAblErrorHeapDealocForPmuSramMsgBlock",
            "Heap Dealloc for PMU SRAM Msg Block error",
        ),
        0xEA00E2BF => ("TpAblErrorDdrRecovery", "DDR Recovery error"),
        0xEA00EBC0 => ("TpAblErrorRrwTest", "RRW Test error"),
        0xEA00E2C1 => ("TpAblErrorOdtInit", "On Die Thermal error"),
        0xEA00E2C2 => (
            "TpAblErrorHeapAllocForDctStructAndChDefStruct",
            "Heap Alloc for DCT/ChDef Struct error",
        ),
        0xEA00E2C3 => (
            "TpAblErrorHeapAlocForPmuSramMsgBlock",
            "Heap Alloc for PMU SRAM Msg Block error",
        ),
        0xEA00E2C4 => {
            ("TpAblErrorHeapPhyPllLockFailure", "Phy PLL Lock Failure")
        }
        0xEA00E2C5 => ("TpAblErrorPmuTraining", "PMU Training error"),
        0xEA00E2C6 => (
            "TpAblErrorFailureToLoadOrVerifyPmuFw",
            "Failure to Load/Verify PMU FW",
        ),
        0xEA00E2C7 => (
            "TpAblErrorAllocateForPmuSramMsgBlockNoInit",
            "Allocate PMU SRAM Msg Block No Init error",
        ),
        0xEA00E2C8 => (
            "TpAblErrorFailureBiosPmuFwMismatchAgesaPmuFwVersion",
            "BIOS PMU FW / AGESA PMU FW version mismatch",
        ),
        0xEA00E2C9 => ("TpAblErrorAgesaMemoryTest", "AGESA memory test error"),
        0xEA00E2CA => (
            "TpAblErrorDeallocateForPmuSramMsgBlock",
            "Dealloc PMU SRAM Msg Block error",
        ),
        0xEA00E2CB => {
            ("TpAblErrorModuleTypeMismatchRDimm", "Module Type Mismatch RDIMM")
        }
        0xEA00E2CC => (
            "TpAblErrorModuleTypeMismatchLRDimm",
            "Module Type Mismatch LRDIMM",
        ),
        0xEA00E2CD => ("TpAblErrorMemAutoNvdimm", "Mem Auto NVDIMM error"),
        0xEA00E2CE => ("TpAblErrorUnknownResponse", "Unknown Response error"),
        0xEA00E2D0 => (
            "TpAblErrorOverClockErrorPmuTraining",
            "Overclock PMU Training error",
        ),
        0xEA00E2D1 => ("TpAblErrorAbl1GenError", "ABL1 General Error"),
        0xEA00E2D2 => ("TpAblErrorAbl2GenError", "ABL2 General Error"),
        0xEA00E2D3 => ("TpAblErrorAbl3GenError", "ABL3 General Error"),
        0xEA00E2D4 => ("TpAblErrorAbl5GenError", "ABL5 General Error"),
        0xEA00E2D5 => {
            ("TpAblErrorOverClockMemInit", "Overclock Mem Init Error")
        }
        0xEA00E2D6 => {
            ("TpAblErrorOverClockMemOther", "Overclock Mem Other Error")
        }
        0xEA00E2D7 => ("TpAblErrorAbl6GenError", "ABL6 General Error"),
        0xEA00E2D8 => ("TpEventLogInit", "Event Log Error"),
        0xEA00E2D9 => ("TpAblErrorAbl1FatalError", "FATAL ABL1 Error"),
        0xEA00E2DA => ("TpAblErrorAbl2FatalError", "FATAL ABL2 Error"),
        0xEA00E2DB => ("TpAblErrorAbl3FatalError", "FATAL ABL3 Error"),
        0xEA00E2DC => ("TpAblErrorAbl4FatalError", "FATAL ABL4 Error"),
        0xEA00E2DD => (
            "TpAblErrorSlaveSyncFunctionExecutionError",
            "Slave Sync function execution Error",
        ),
        0xEA00E2DE => (
            "TpAblErrorSlaveSyncCommWithDataSentToMasterError",
            "Slave Sync comm data sent to master Error",
        ),
        0xEA00E2DF => (
            "TpAblErrorSlaveBroadcastCommFromMasterToSlaveError",
            "Slave broadcast comm master->slave Error",
        ),
        0xEA00E2E0 => ("TpAblErrorAbl6FatalError", "FATAL ABL6 Error"),
        0xEA00E2E1 => ("TpAblErrorSlaveOfflineMsgError", "Slave Offline Error"),
        0xEA00E2E2 => (
            "TpAblErrorSlaveInformsMasterErrorInoError",
            "Slave Informs Master Error Info Error",
        ),
        0xEA00E2E3 => (
            "TpAblErrorHeapLocateForPmuSramMsgBlock",
            "Heap Locate for PMU SRAM Msg Block Error",
        ),
        0xEA00E2E4 => ("TpAblErrorAbl2AutoAloc", "ABL2 Auto Alloc Error"),
        0xEA00E2E5 => (
            "TpAblErrorFlowP3FamilySupport/TpAblErrorAbl4GenError",
            "Flow P3 Family Support / ABL 4 General Error",
        ),
        0xEA00E2E7 => (
            "TpAblErrorMixRdimmLrdimmInChannel",
            "Mix RDIMM & LRDIMM in a channel",
        ),
        0xEA00E2E8 => (
            "TpAblErrorMemoryPresentOnDisconnected",
            "Memory Present on Disconnected Channel",
        ),
        0xEA00E2EB => {
            ("TpAblErrorMbistHeapAlloc", "MBIST Heap Allocation Error")
        }
        0xEA00E2EC => ("TpAblErrorMbistResultsError", "MBIST Results Error"),
        0xEA00E2ED => {
            ("TpAblErrorNoDimmSmbusInfoError", "No DIMM SMBus Info Error")
        }
        0xEA00E2EE => {
            ("TpAblErrorPorMaxFreqTblError", "POR Max Freq Table Error")
        }
        0xEA00E2EF => (
            "TpAblErrorUnsupportedDimmConfuglError",
            "Unsupported DIMM Config Error",
        ),
        0xEA00E2F0 => ("TpAblErrorNoPsTableError", "No Ps Table Error"),
        0xEA00E2F1 => (
            "TpAblErrorCadBusTmgNoFoundError",
            "CAD Bus Timing Not Found Error",
        ),
        0xEA00E2F2 => (
            "TpAblErrorDataBusTmgNoFoundError",
            "Data Bus Timing Not Found Error",
        ),
        0xEA00E2F3 => {
            ("TpAblErrorLrIbtNotFoundError", "LRDIMM IBT Not Found Error")
        }
        0xEA00E2F4 => (
            "TpAblErrorUnsupportedDimmConfigMaxFreqError",
            "Unsupported DIMM Config Max Freq Error",
        ),
        0xEA00E2F5 => ("TpAblErrorMr0NotFoundError", "MR0 Not Found Error"),
        0xEA00E2F6 => {
            ("TpAblErrorOdtPAtNotFoundError", "ODT Pattern Not Found Error")
        }
        0xEA00E2F7 => (
            "TpAblErrorRc10OpSpeedNotFoundError",
            "RC10 Op Speed Not Found Error",
        ),
        0xEA00E2F8 => {
            ("TpAblErrorRc2IbtNotFoundError", "RC2 IBT Not Found Error")
        }
        0xEA00E2F9 => ("TpAblErrorRttNotFoundError", "RTT Not Found Error"),
        0xEA00E2FA => {
            ("TpAblErrorChecksumReStrtError", "Checksum ReStrt Error")
        }
        0xEA00E2FB => ("TpAblErrorNoChipselectError", "No Chipselect Error"),
        0xEA00E2FC => {
            ("TpAblErrorNoCommonCasLAtError", "No Common CAS Latency Error")
        }
        0xEA00E2FD => (
            "TpAblErrorCasLatXceedsTaaMaxError",
            "CAS Latency exceeds Taa Max Error",
        ),
        0xEA00E2FE => (
            "TpAblErrorNvdimmArmMissmatcPowerPolicyError",
            "NVDIMM Arm Mismatch Power Policy Error",
        ),
        0xEA00E2FF => (
            "TpAblErrorNvdimmArmMissmatchPowerSouceError",
            "NVDIMM Arm Mismatch Power Source Error",
        ),
        0xEA00E300 => ("TpAblErrorAbl1MemInitError", "ABL 1 Mem Init Error"),
        0xEA00E301 => ("TpAblErrorAbl2MemInitError", "ABL 2 Mem Init Error"),
        0xEA00E302 => ("TpAblErrorAbl4MemInitError", "ABL 4 Mem Init Error"),
        0xEA00E303 => ("TpAblErrorAbl6MemInitError", "ABL 6 Mem Init Error"),
        0xEA00E304 => {
            ("TpAblErrorAbl1ErrorReportError", "ABL 1 Error Report Error")
        }
        0xEA00E305 => {
            ("TpAblErrorAbl2ErrorReportError", "ABL 2 Error Report Error")
        }
        0xEA00E306 => {
            ("TpAblErrorAbl3ErrorReportError", "ABL 3 Error Report Error")
        }
        0xEA00E307 => {
            ("TpAblErrorAbl4ErrorReportError", "ABL 4 Error Report Error")
        }
        0xEA00E308 => {
            ("TpAblErrorAbl6ErrorReportError", "ABL 6 Error Report Error")
        }
        0xEA00E309 => {
            ("TpAblErrorAbl7ErrorReportError", "ABL 7 Error Report Error")
        }
        0xEA00E30A => (
            "TpAblErrorMsgSlaveSyncFunctionExecutionError",
            "Slave Sync Function Execution Error",
        ),
        0xEA00E30B => ("TpAblErrorSlaveOfflineError", "Slave Offline Error"),
        0xEA00E30C => ("TpAblErrorSyncMasterError", "Sync Master Error"),
        0xEA00E30D => (
            "TpAblErrorSlaveInformsMasterInfoMsgError",
            "Slave Informs Master Info Msg Error",
        ),
        0xEA00E30E => {
            ("TpAblErrorMemLrdimmMixCfgError", "Mix vendor LRDIMM in channel")
        }
        0xEA00E30F => ("TpAblErrorGenAssertError", "General Assert Error"),
        0xEA00E310 => (
            "TpAblErrorNoDimmOnAnyChannelInSystem",
            "No DIMMs on any channel in system",
        ),
        0xEA00E311 => {
            ("TpAblErrorSharedHeapAlocError", "Shared Heap Alloc error")
        }
        0xEA00E312 => ("TpAblErrorMainHeapAlocError", "Main Heap Alloc error"),
        0xEA00E313 => {
            ("TpAblErrorSharedAutolocError", "Shared Heap Locate error")
        }
        0xEA00E314 => ("TpAblErrorMainAutolocError", "Main Heap Locate error"),
        0xEA00E316 => (
            "TpAblErrorNoMemoryAvailableInSystem",
            "No memory available in system",
        ),
        0xEA00E320 => (
            "TpAblErrorMixedEccAndNonEccDimmInChannel",
            "Mixed ECC and Non-ECC DIMM in channel",
        ),
        0xEA00E321 => (
            "TpAblErrorMixed3DSAndNon3DSDimmInChannel",
            "Mixed 3DS and Non-3DS DIMM in channel",
        ),
        0xEA00E322 => (
            "TpAblErrorMixedX4AndX8DimmInChannel",
            "Mixed x4 and x8 DIMM in channel",
        ),
        0xEA00E327 => (
            "TpAblErrorS0i3DfRestoreBufferError",
            "S0i3 DF restore buffer Error",
        ),
        0xEA00E328 => (
            "TpAblErrorCpuOPNMismatchInSockets",
            "CPU OPN Mismatch in Multi Socket",
        ),
        0xEA00E329 => (
            "TpProcRecoverableApcbChecksumError",
            "Recoverable APCB Checksum Error",
        ),
        0xEA00E32A => {
            ("TpProcFatalApcbChecksumError", "Fatal APCB Checksum Error")
        }
        0xEA00E32B => ("TpAblErrorBistFailure", "BIST Failure"),
        0xEA00E32C => {
            ("TpAblErrorDdrTypeMismatchDdr5", "DDR Type Mismatch DDR5")
        }
        0xEA00E32D => (
            "TpAblErrorMixDifferentEccSizeDimmInChannel",
            "Mix DIMM with different ECC bit size in channel",
        ),
        0xEA00E32E => (
            "TpAblErrorCantRecoverI2cBus",
            "Cannot recover I2C bus (needs power cycle)",
        ),
        0xEA00E32F => ("TpAblErrorI2cResetFailure", "I2C reset failure"),
        0xEA00E330 => {
            ("TpAblErrorModuleTypeMismatch", "DDR Module Type Mismatch")
        }
        0xEA00E331 => ("TpAblErrorDimmPmicError", "PMIC Error"),
        0xEA00E332 => ("TpAblErrorIncompatibleOPN", "Incompatible OPN"),
        0xEA00E333 => (
            "TpAblErrorCantRecoverI3cBus",
            "Cannot recover I3C bus (needs power cycle)",
        ),
        0xEA00E334 => ("TpAblErrorI3cResetFailure", "I3C reset failure"),
        0xEA00E335 => (
            "TpAblErrorAbsenceAcPowerOrWlanApcbData",
            "Missing AC-Power/WLAN GPIO APCB Data",
        ),
        0xEA00E33C => (
            "TpAblErrorDimmWithSpecificVendorRcdVersion",
            "DIMM with RCD Montage version B1 detected",
        ),
        0xEA00E33F => (
            "TpAblErrorDimmWithSpecificPMICVendorVersion",
            "DIMM with TI PMIC rev 1.1 (XTPS) detected",
        ),
        0xEA00E345 => (
            "TpAblErrorUnsupportedModTypeCDimm",
            "CDimm/Socdimm module type detected",
        ),
        0xEA00E346 => {
            ("TpAblError3DSDimmInSp6System", "3DS DIMM in SP6 system")
        }
        0xEA00E347 => ("TpAblErrorSelfHealingBist", "Self-Healing BIST Error"),
        0xEA00E360 => ("TpAblErrorAPCBBoardId", "APCB board id check failure"),

        // ABL 1b Special Flow
        0xEA00E2B1 => {
            ("TpAblErrorAbl1bAutoAloc", "ABL1b Auto Allocation error")
        }
        0xEA00E2B2 => {
            ("TpAblErrorAbl1bNoNbConst", "ABL1b No NB Constructor error")
        }
        0xEA00E2B6 => ("TpAblErrorAbl1bGen", "ABL1b General error"),

        // ABL Functions & End Marker
        0xEA00E60B => {
            ("TpAblFunctionsExecutionBefore", "ABL Functions execute Before")
        }
        0xEA00E60C => ("TpAblFunctionsExecutionStart", "ABL Functions execute"),
        0xEA10AD68 => ("TpAblLoadApcb", "ABL Load APCB"),

        _ => return None,
    })
}
