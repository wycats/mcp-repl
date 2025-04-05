use nu_command::*;
use nu_protocol::engine::{EngineState, StateWorkingSet};

pub fn add_shell_command_context(mut engine_state: EngineState) -> EngineState {
    let delta = {
        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_command {
            ( $( $command:expr ),* $(,)? ) => {
                $( working_set.add_decl(Box::new($command)); )*
            };
        }

        // If there are commands that have the same name as default declarations,
        // they have to be registered before the main declarations. This helps to make
        // them only accessible if the correct input value category is used with the
        // declaration

        // Charts
        bind_command! {
            Histogram
        }

        // Filters
        bind_command! {
            Shuffle,
        }
        bind_command! {
            All,
            Any,
            Append,
            Chunks,
            Columns,
            Compact,
            Default,
            Drop,
            DropColumn,
            DropNth,
            Each,
            Enumerate,
            Every,
            Filter,
            Find,
            First,
            Flatten,
            Get,
            GroupBy,
            Headers,
            Insert,
            IsEmpty,
            IsNotEmpty,
            Interleave,
            Items,
            Join,
            Take,
            Merge,
            MergeDeep,
            Move,
            TakeWhile,
            TakeUntil,
            Last,
            Length,
            Lines,
            ParEach,
            ChunkBy,
            Prepend,
            Reduce,
            Reject,
            Rename,
            Reverse,
            Select,
            Skip,
            SkipUntil,
            SkipWhile,
            Slice,
            Sort,
            SortBy,
            SplitList,
            Tee,
            Transpose,
            Uniq,
            UniqBy,
            Upsert,
            Update,
            Values,
            Where,
            Window,
            Wrap,
            Zip,
        };

        // Misc
        bind_command! {
            Panic,
            Source,
            Tutor,
        };

        // Path
        bind_command! {
            Path,
            PathBasename,
            PathSelf,
            PathDirname,
            PathExists,
            PathExpand,
            PathJoin,
            PathParse,
            PathRelativeTo,
            PathSplit,
            PathType,
        };

        // Help
        bind_command! {
            Help,
            HelpAliases,
            HelpExterns,
            HelpCommands,
            HelpModules,
            HelpOperators,
            HelpPipeAndRedirect,
            HelpEscapes,
        };

        // Debug
        bind_command! {
            Ast,
            Debug,
            DebugInfo,
            DebugProfile,
            Explain,
            Inspect,
            Metadata,
            MetadataAccess,
            MetadataSet,
            TimeIt,
            View,
            ViewBlocks,
            ViewFiles,
            ViewIr,
            ViewSource,
            ViewSpan,
        };

        // Strings
        bind_command! {
            Char,
            Decode,
            Encode,
            DecodeHex,
            EncodeHex,
            DecodeBase32,
            EncodeBase32,
            DecodeBase32Hex,
            EncodeBase32Hex,
            DecodeBase64,
            EncodeBase64,
            DetectColumns,
            Parse,
            Split,
            SplitChars,
            SplitColumn,
            SplitRow,
            SplitWords,
            Str,
            StrCapitalize,
            StrContains,
            StrDistance,
            StrDowncase,
            StrEndswith,
            StrExpand,
            StrJoin,
            StrReplace,
            StrIndexOf,
            StrLength,
            StrReverse,
            StrStats,
            StrStartsWith,
            StrSubstring,
            StrTrim,
            StrUpcase,
            Format,
            FormatDate,
            FormatDuration,
            FormatFilesize,
        };

        // Date
        bind_command! {
            Date,
            DateHumanize,
            DateListTimezones,
            DateNow,
            DateToTimezone,
        };

        // Shells
        bind_command! {
            Exit,
        };

        // Formats
        bind_command! {
            From,
            FromCsv,
            FromJson,
            FromMsgpack,
            FromMsgpackz,
            FromNuon,
            FromOds,
            FromSsv,
            FromToml,
            FromTsv,
            FromXlsx,
            FromXml,
            FromYaml,
            FromYml,
            To,
            ToCsv,
            ToJson,
            ToMd,
            ToMsgpack,
            ToMsgpackz,
            ToNuon,
            ToText,
            ToToml,
            ToTsv,
            Upsert,
            Where,
            ToXml,
            ToYaml,
            ToYml,
        };

        // Viewers
        bind_command! {
            Griddle,
            Table,
        };

        // Conversions
        bind_command! {
            Fill,
            Into,
            IntoBool,
            IntoBinary,
            IntoCellPath,
            IntoDatetime,
            IntoDuration,
            IntoFloat,
            IntoFilesize,
            IntoInt,
            IntoRecord,
            IntoString,
            IntoGlob,
            IntoValue,
            SplitCellPath,
        };

        // Env
        bind_command! {
            ExportEnv,
            LoadEnv,
            SourceEnv,
            WithEnv,
            ConfigNu,
            ConfigEnv,
            ConfigFlatten,
            ConfigMeta,
            ConfigReset,
            ConfigUseColors,
        };

        // Math
        bind_command! {
            Math,
            MathAbs,
            MathAvg,
            MathCeil,
            MathFloor,
            MathMax,
            MathMedian,
            MathMin,
            MathMode,
            MathProduct,
            MathRound,
            MathSqrt,
            MathStddev,
            MathSum,
            MathVariance,
            MathLog,
        };

        // Bytes
        bind_command! {
            Bytes,
            BytesLen,
            BytesSplit,
            BytesStartsWith,
            BytesEndsWith,
            BytesReverse,
            BytesReplace,
            BytesAdd,
            BytesAt,
            BytesIndexOf,
            BytesCollect,
            BytesRemove,
            BytesBuild
        }

        bind_command! {
            Url,
            UrlBuildQuery,
            UrlSplitQuery,
            UrlDecode,
            UrlEncode,
            UrlJoin,
            UrlParse,
        }

        // Random
        bind_command! {
            Random,
            RandomBool,
            RandomChars,
            RandomDice,
            RandomFloat,
            RandomInt,
            RandomUuid,
            RandomBinary
        };

        // Generators
        bind_command! {
            Cal,
            Seq,
            SeqDate,
            SeqChar,
            Generate,
        };

        // Hash
        bind_command! {
            Hash,
            HashMd5::default(),
            HashSha256::default(),
        };

        // Experimental
        bind_command! {
            IsAdmin,
            JobSpawn,
            JobList,
            JobKill,
            Job,
        };

        // Removed
        bind_command! {
            LetEnv,
            DateFormat,
        };

        working_set.render()
    };

    if let Err(err) = engine_state.merge_delta(delta) {
        eprintln!("Error creating default context: {err:?}");
    }

    // Cache the table decl id so we don't have to look it up later
    let table_decl_id = engine_state.find_decl("table".as_bytes(), &[]);
    engine_state.table_decl_id = table_decl_id;

    engine_state
}
