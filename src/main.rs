use std::path::PathBuf;

use clap::Parser;

mod parser;

use parser::{partitions::is_filesystem_partition, qnx6::parse_qnx6};

#[derive(Debug, Parser)]
#[command(author, version, about = "Extract files from a QNX6 disk image")]
struct Cli {
    /// Path to the source disk image.
    image: PathBuf,

    /// Directory where extracted files will be written.
    #[arg(short, long, value_name = "DIR")]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    if !cli.image.is_file() {
        return Err(format!(
            "disk image does not exist or is not a file: {}",
            cli.image.display()
        ));
    }

    if cli.output.exists() && !cli.output.is_dir() {
        return Err(format!(
            "output destination exists but is not a directory: {}",
            cli.output.display()
        ));
    }

    std::fs::create_dir_all(&cli.output).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            cli.output.display()
        )
    })?;

    println!("====================");
    println!("Input");
    println!("====================");
    println!("image: {}", cli.image.display());
    println!("output: {}", cli.output.display());
    println!();

    println!("====================");
    println!("Partition Scan");
    println!("====================");
    let partition_table = parse_qnx6(&cli.image)?;

    println!();
    println!("====================");
    println!("Partition Summary");
    println!("====================");
    let filesystem_partitions: Vec<_> = partition_table
        .partitions
        .iter()
        .filter(|partition| is_filesystem_partition(partition))
        .collect();

    println!("table: {:?}", partition_table.kind);
    println!("filesystems: {}", filesystem_partitions.len());
    println!();

    for partition in filesystem_partitions {
        println!("partition {}", partition.index);
        println!("  type: {}", partition.partition_type);
        println!("  first_lba: {}", partition.first_lba);
        println!("  sectors: {}", partition.sector_count);
        println!("  byte_offset: {}", partition.byte_offset);
        println!("  byte_len: {}", partition.byte_len);
        println!();
    }

    println!("====================");
    println!("Done");
    println!("====================");
    println!(
        "detected {:?} partition table with {} filesystem partition(s)",
        partition_table.kind,
        partition_table
            .partitions
            .iter()
            .filter(|partition| is_filesystem_partition(partition))
            .count()
    );

    Ok(())
}
