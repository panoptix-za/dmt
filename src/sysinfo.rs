extern crate raw_cpuid;
extern crate os_info;


#[cfg(test)]
mod tests {
    use super::*;
    use self::raw_cpuid::CpuId;

    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_raw_cpuid() {
        let cpuid = CpuId::new();

        match cpuid.get_vendor_info() {
            Some(vf) => assert!(vf.as_string() == "GenuineIntel"),
            None => ()
        }

        let has_sse = match cpuid.get_feature_info() {
            Some(finfo) => finfo.has_sse(),
            None => false
        };

        if has_sse {
//            println!("CPU supports SSE!");
        }

        match cpuid.get_cache_parameters() {
            Some(cparams) => {
                for cache in cparams {
                    let size = cache.associativity() * cache.physical_line_partitions() * cache.coherency_line_size() * cache.sets();
//                    println!("L{}-Cache size is {}", cache.level(), size);
                }
            },
            None => () // println!("No cache parameter information available"),
        }
    }

    #[test]
    fn test_os_info() {
        let os = os_info::get();

        // Print information separately:
//        println!("Type: {}", os.os_type());
//        println!("Version: {}", os.version());
    }

}